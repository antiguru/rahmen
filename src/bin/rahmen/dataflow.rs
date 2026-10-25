use image::{DynamicImage, GenericImageView};
use log::{error, info};
use pathfinder_geometry::rect::RectI;
use rahmen::Vector;
use rahmen::dataflow::{Configuration, FormatText, ResizeImage};
use rahmen::errors::RahmenResult;
use rahmen::font::FontRenderer;
use rahmen::provider::{Provider, StatusLineFormatter, load_image_from_path};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use timely::container::CapacityContainerBuilder;
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::capture::Event;
use timely::dataflow::operators::vec::{Branch, Filter, Map, ResultStream};
use timely::dataflow::operators::{
    Capture, Concat, ConnectLoop, Enter, Inspect, Leave, LoopVariable, Notificator, Operator, Probe,
};
use timely::dataflow::{InputHandle, ProbeHandle};
use timely::order::Product;
use timely::worker::Worker;

/// dataflow control, this is used as result R part
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum RunControl {
    /// terminate stream processing (by external command)
    Terminate,
    /// stream processing encountered an error, but will continue
    Suppressed,
}

/// error handler for display stuff
fn fatal_err<T>(result: RahmenResult<Option<T>>) -> RunResult<T> {
    match result {
        // empty result means we terminate as planned (e.g. end of list)
        Ok(None) => Err(RunControl::Terminate),
        // we process the result
        Ok(Some(t)) => Ok(t),
        // display error and terminate
        Err(e) => {
            error!("Encountered error, terminating: {}", e);
            Err(RunControl::Terminate)
        }
    }
}

/// to keep running the dataflow when there's a stream error
fn suppress_err<T>(result: RahmenResult<T>) -> RunResult<T> {
    result.map_err(|e| {
        // just notify about error but keep processing
        error!("Encountered error, suppressing: {}", e);
        RunControl::Suppressed
    })
}

// `DynamicImage` only implements `PartialEq` (not `Eq`) since it can hold floating-point pixels.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Render {
    Image(usize, Vector, Arc<DynamicImage>),
    Blank(usize, Vector, Vector),
}

type RunResult<T> = Result<T, RunControl>;

pub(super) struct Options {
    pub buffer_max_size: usize,
    pub show_time: bool,
    pub time_format: String,
}

pub(super) fn build(
    worker: &mut Worker,
    input_configuration: &mut InputHandle<Duration, CapacityContainerBuilder<Vec<Configuration>>>,
    probe: &ProbeHandle<Duration>,
    mut provider: Box<dyn Provider<PathBuf>>,
    status_line_formatter: StatusLineFormatter,
    font_renderer: FontRenderer,
    options: Options,
) -> std::sync::mpsc::Receiver<Event<Duration, Vec<RunResult<Render>>>> {
    let Options {
        buffer_max_size,
        show_time,
        time_format,
    } = options;
    worker.dataflow(|scope| {
        let configuration_stream = input_configuration.to_stream(scope);

        let img_path_stream = scope.scoped::<Product<_, u32>, _, _>("File loading", |inner| {
            let (handle, cycle) = inner.loop_variable(1);
            let (ok, err) = configuration_stream
                .clone()
                .filter(|c| matches!(c, Configuration::Tick))
                .enter(inner)
                .concat(cycle)
                // obtain next path
                .map(move |_| fatal_err(provider.next_image()))
                // Load image
                .and_then(move |ref path| {
                    suppress_err(
                        load_image_from_path(path, Some(buffer_max_size))
                            .map(|img| (path.clone(), Arc::new(img))),
                    )
                })
                .branch(|_t, d| d.as_ref().err() == Some(&RunControl::Suppressed));
            err.map(|_| Configuration::Tick).connect_loop(handle);
            ok.leave(scope)
        });
        let err_stream = img_path_stream.clone().err();

        let mut stash: HashMap<Duration, String> = HashMap::new();
        let mut current_text = None;

        let mut status_line_stream = img_path_stream
            .clone()
            .ok()
            .flat_map(move |(p, _img)| status_line_formatter.format(&p).ok())
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Greeting(text) => Some(text),
                _ => None,
            }))
            .inspect(|loc| info!("Status line: {}", loc));
        if show_time {
            status_line_stream = status_line_stream.unary_notify(
                Pipeline,
                "Show time",
                Some(Duration::from_secs(0)),
                move |input, output, not: &mut Notificator<Duration>| {
                    input.for_each(|cap, data| {
                        if let Some(text) = data.drain(..).next_back() {
                            *stash.entry(*cap.time()).or_default() = text;
                            not.notify_at(cap.retain(0));
                        }
                    });
                    not.for_each(|cap, cnt, not| {
                        let request_notification = if let Some(text) = stash.remove(cap.time()) {
                            current_text = Some(text);
                            cnt == 2
                        } else {
                            true
                        };
                        let now = chrono::Local::now();
                        let delay = std::cmp::max(50, 1000 - now.timestamp_subsec_millis() as u64);
                        if request_notification && !not.frontier(0).is_empty() {
                            let mut next_time = *cap.time() + Duration::from_millis(delay);
                            while !not.frontier(0).less_equal(&next_time) {
                                next_time += Duration::from_secs(1);
                            }
                            not.notify_at(cap.delayed(&next_time));
                        }
                        if let Some(text) = &current_text {
                            let time_text = format!("[{}] {}", now.format(&time_format), text);
                            output.session(&cap).give(time_text);
                        }
                    });
                },
            );
        }
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
                                                height
                                                    - (current_font_size.unwrap_or(0.)
                                                        * current_font_canvas_vstretch
                                                            .unwrap_or(1.0))
                                                    .ceil()
                                                        as u32,
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

        let img_stream = img_path_stream
            .clone()
            .ok()
            .map(|(_, img)| img)
            .concat(configuration_stream.clone().flat_map(|c| match c {
                Configuration::Splash(img) => Some(img),
                _ => None,
            }))
            .resize_image(&adjusted_configuration_stream, 1);

        let mut size_stash: HashMap<usize, _> = HashMap::new();
        let mut input_buffer: HashMap<_, Vec<(_, _, _)>> = HashMap::new();

        let composed_img_stream = img_stream.concat(text_img_stream).unary_notify(
            Pipeline,
            "Infer blanking",
            None,
            move |input, output, not| {
                input.for_each(|time, data| {
                    input_buffer.entry(*time.time()).or_default().append(data);
                    not.notify_at(time.retain(0));
                });
                not.for_each(|time, _count, _not| {
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
