use image::{DynamicImage, GenericImageView};
use log::info;
use pathfinder_geometry::rect::RectI;
use rahmen::Vector;
use rahmen::dataflow::{Configuration, FormatText, ResizeImage};
use rahmen::font::FontRenderer;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::operators::vec::Map;
use timely::dataflow::operators::{Capture, Concat, Notificator, Operator, Probe};
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::worker::Worker;

use super::prefetch::Prepared;
pub(super) use super::prefetch::RunControl;

#[derive(Clone, Debug)]
pub(super) enum Update {
    Image(Prepared),
    Loading(bool),
    Terminate,
}

#[derive(Clone)]
enum StatusUpdate {
    Text(String),
    Loading(bool),
}

// `DynamicImage` only implements `PartialEq` (not `Eq`) since it can hold floating-point pixels.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Render {
    Image(usize, Vector, Arc<DynamicImage>),
    Blank(usize, Vector, Vector),
}

type RunResult<T> = Result<T, RunControl>;

pub(super) struct Options {
    pub loading_marker: &'static str,
    pub show_time: bool,
    pub time_format: String,
}

pub(super) fn build(
    worker: &mut Worker,
    input_configuration: &mut InputHandle<Duration, CapacityContainerBuilder<Vec<Configuration>>>,
    probe: &ProbeHandle<Duration>,
    input_updates: &mut InputHandle<Duration, CapacityContainerBuilder<Vec<Update>>>,
    font_renderer: FontRenderer,
    options: Options,
) -> std::sync::mpsc::Receiver<Event<Duration, Vec<RunResult<Render>>>> {
    let Options {
        loading_marker,
        show_time,
        time_format,
    } = options;
    worker.dataflow(|scope| {
        let configuration_stream = input_configuration.to_stream(scope);

        let updates = input_updates.to_stream(scope);
        let err_stream = updates.clone().flat_map(|update| match update {
            Update::Terminate => Some(RunControl::Terminate),
            _ => None,
        });
        let mut stash: HashMap<Duration, Vec<StatusUpdate>> = HashMap::new();
        let mut current_text = None;
        let mut loading = false;
        let mut next_tick = show_time.then_some(Duration::ZERO);
        let status_line_stream = updates
            .clone()
            .flat_map(|update| match update {
                Update::Image(prepared) => prepared.status.map(StatusUpdate::Text),
                Update::Loading(loading) => Some(StatusUpdate::Loading(loading)),
                Update::Terminate => None,
            })
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Greeting(text) => Some(StatusUpdate::Text(text)),
                _ => None,
            }))
            .unary_notify(
                Pipeline,
                "Show time and loading",
                next_tick,
                move |input, output, not: &mut Notificator<Duration>| {
                    input.for_each(|cap, data| {
                        stash.entry(*cap.time()).or_default().append(data);
                        not.notify_at(cap.retain(0));
                    });
                    not.for_each(|cap, _, not| {
                        if let Some(updates) = stash.remove(cap.time()) {
                            for update in updates {
                                match update {
                                    StatusUpdate::Text(text) => {
                                        info!("Status line: {}", text);
                                        current_text = Some(text);
                                    }
                                    StatusUpdate::Loading(value) => loading = value,
                                }
                            }
                        }
                        let now = chrono::Local::now();
                        // Only the outstanding periodic notification schedules another tick.
                        // Text/loading notifications at the same time cannot multiply the clock.
                        if next_tick == Some(*cap.time()) {
                            next_tick = None;
                            if !not.frontier(0).is_empty() {
                                let delay =
                                    std::cmp::max(50, 1000 - now.timestamp_subsec_millis() as u64);
                                let mut next = *cap.time() + Duration::from_millis(delay);
                                while !not.frontier(0).less_equal(&next) {
                                    next += Duration::from_secs(1);
                                }
                                next_tick = Some(next);
                                not.notify_at(cap.delayed(&next));
                            }
                        }
                        if let Some(text) = &current_text {
                            let timestamp = show_time.then(|| now.format(&time_format).to_string());
                            output.session(&cap).give(status_text(
                                timestamp.as_deref(),
                                text,
                                loading,
                                loading_marker,
                            ));
                        }
                    });
                },
            );
        let status_line_stream =
            status_line_stream.map(|s| s.split('\n').map(Into::into).collect());

        let text_img_stream =
            status_line_stream.format_text(&configuration_stream, font_renderer, 2);

        let adjusted_configuration_stream = {
            let mut stash: HashMap<_, Vec<_>> = HashMap::new();
            // Hack: adjust screen size for the resize operator to reserve space for the status line
            let mut current_font_size = None;
            let mut current_font_canvas_vstretch = None;

            configuration_stream.clone().unary_notify(
                Pipeline,
                "Adjust configuration",
                None,
                move |input, output, not| {
                    input.for_each(|cap, data| {
                        stash.entry(*cap.time()).or_default().append(data);
                        not.notify_at(cap.retain(0));
                    });
                    not.for_each(|cap, _, _not| {
                        if let Some(updates) = stash.remove(cap.time()) {
                            output.session(&cap).give_iterator(updates.into_iter().map(
                                |configuration| {
                                    match configuration {
                                        Configuration::FontSize(font_size) => {
                                            current_font_size = Some(font_size);
                                            Configuration::FontSize(font_size)
                                        }
                                        Configuration::FontCanvasVStretch(font_canvas_vstretch) => {
                                            current_font_canvas_vstretch =
                                                Some(font_canvas_vstretch);
                                            Configuration::FontCanvasVStretch(font_canvas_vstretch)
                                        }
                                        Configuration::ScreenDimensions(width, height) => {
                                            Configuration::ScreenDimensions(
                                                width,
                                                height.saturating_sub(
                                                    (current_font_size.unwrap_or(0.)
                                                        * current_font_canvas_vstretch
                                                            .unwrap_or(1.0))
                                                    .ceil()
                                                        as u32,
                                                ),
                                            )
                                        }
                                        configuration => configuration,
                                    }
                                },
                            ));
                        }
                    });
                },
            )
        };

        let img_stream = updates
            .flat_map(|update| match update {
                Update::Image(prepared) => Some(prepared.image),
                _ => None,
            })
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Splash(img) => Some(img),
                _ => None,
            }))
            .resize_image(&adjusted_configuration_stream, 1);

        let mut size_stash: HashMap<usize, _> = HashMap::new();
        let mut input_buffer: HashMap<_, Vec<(_, _, _)>> = HashMap::new();
        let mut viewport_stash: HashMap<_, Vec<_>> = HashMap::new();
        let mut viewport = None;

        let composed_img_stream = img_stream.concat(text_img_stream).binary_notify(
            configuration_stream.clone(),
            Pipeline,
            Pipeline,
            "Infer blanking",
            None,
            move |input, configuration, output, not| {
                input.for_each(|time, data| {
                    input_buffer.entry(*time.time()).or_default().append(data);
                    not.notify_at(time.retain(0));
                });
                configuration.for_each(|time, data| {
                    viewport_stash.entry(*time.time()).or_default().append(data);
                    not.notify_at(time.retain(0));
                });
                not.for_each(|time, _count, _not| {
                    if let Some(configurations) = viewport_stash.remove(time.time()) {
                        for configuration in configurations {
                            if let Configuration::ScreenDimensions(width, height) = configuration {
                                viewport = Some(RectI::new(
                                    Vector::new(0, 0),
                                    Vector::new(width as _, height as _),
                                ));
                            }
                        }
                    }
                    if let Some(updates) = input_buffer.remove(time.time()) {
                        output
                            .session(&time)
                            .give_iterator(updates.into_iter().flat_map(|(key, anchor, img)| {
                                let rect = RectI::new(
                                    anchor,
                                    Vector::new(img.dimensions().0 as _, img.dimensions().1 as _),
                                );
                                size_stash
                                    .insert(key, rect)
                                    .into_iter()
                                    .flat_map(move |old_rect| compute_blanking(key, rect, old_rect))
                                    .filter_map(|render| {
                                        // Old rectangles can extend beyond a newly reduced viewport.
                                        if let Render::Blank(key, origin, size) = render {
                                            let clipped =
                                                RectI::new(origin, size).intersection(viewport?)?;
                                            (clipped.width() > 0 && clipped.height() > 0).then(
                                                || {
                                                    Render::Blank(
                                                        key,
                                                        clipped.origin(),
                                                        clipped.size(),
                                                    )
                                                },
                                            )
                                        } else {
                                            Some(render)
                                        }
                                    })
                                    .chain(Some(Render::Image(key, anchor, img)))
                            }));
                    }
                })
            },
        );

        err_stream
            .map(Err)
            .concat(composed_img_stream.map(Ok))
            .probe_with(probe)
            .capture()
    })
}

pub(super) fn loading_marker(has_glyph: bool) -> &'static str {
    if has_glyph { "◷" } else { "[loading]" }
}

fn status_text(timestamp: Option<&str>, text: &str, loading: bool, marker: &str) -> String {
    let prefix = timestamp
        .map(|time| format!("[{time}] "))
        .unwrap_or_default();
    if loading {
        format!("{prefix}{marker} {text}")
    } else {
        format!("{prefix}{text}")
    }
}

fn compute_blanking(key: usize, rect: RectI, old_rect: RectI) -> Vec<Render> {
    if let Some(overlap) = old_rect.intersection(rect) {
        let above = RectI::from_points(old_rect.origin(), overlap.upper_right());
        let left = RectI::from_points(old_rect.origin(), overlap.lower_left());
        let right = RectI::from_points(overlap.upper_right(), old_rect.lower_right());
        let below = RectI::from_points(overlap.lower_left(), old_rect.lower_right());
        [above, left, right, below]
            .iter()
            .filter(|r| r.width() > 0 && r.height() > 0)
            .map(|r| Render::Blank(key, r.origin(), r.size()))
            .collect::<Vec<_>>()
    } else {
        vec![Render::Blank(key, old_rect.origin(), old_rect.size())]
    }
}

#[cfg(test)]
mod tests;
