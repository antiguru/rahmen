//! Dataflow operators to handle images are required for Rahmen

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::font::FontRenderer;
use crate::picture::Picture;
use crate::{Timer, Vector};
use timely::dataflow::Stream;
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::{Capability, Operator};
use timely::order::TotalOrder;
use timely::progress::Timestamp;
use timely::progress::frontier::MutableAntichain;

/// A stream of images
pub type ImageStream<'a, T> = Stream<'a, T, Vec<Arc<Picture>>>;

/// A keyed stream of offset and image
pub type ImagePosStream<'a, T> = Stream<'a, T, Vec<(usize, Vector, Arc<Picture>)>>;

/// A configuration stream
pub type ConfigurationStream<'a, T> = Stream<'a, T, Vec<Configuration>>;

/// A configuration element to be passed in a configuration stream
#[derive(Debug, Clone)]
pub enum Configuration {
    /// Update the font size for the status line
    FontSize(f32),
    /// factor by which font canvas is higher than font
    FontCanvasVStretch(f32),
    /// Update the screen dimensions
    ScreenDimensions(u32, u32),
    /// Show a new image
    Tick,
    /// Splash screen
    Splash(Arc<Picture>),
    /// Greeting message
    Greeting(String),
}

/// Capabilities held until the input frontiers pass their times, delivered in time order.
///
/// This replaces timely's removed notificator for totally ordered timestamps.
#[derive(Debug)]
pub struct Pending<T: Timestamp> {
    capabilities: BTreeMap<T, Capability<T>>,
}

impl<T: Timestamp> Default for Pending<T> {
    fn default() -> Self {
        Self {
            capabilities: BTreeMap::new(),
        }
    }
}

impl<T: Timestamp + TotalOrder> Pending<T> {
    /// Request delivery of `capability` once the frontiers pass its time.
    pub fn notify_at(&mut self, capability: Capability<T>) {
        self.capabilities
            .entry(capability.time().clone())
            .or_insert(capability);
    }

    /// Return the earliest capability whose time none of `frontiers` can still produce.
    pub fn next(&mut self, frontiers: &[&MutableAntichain<T>]) -> Option<Capability<T>> {
        let entry = self.capabilities.first_entry()?;
        if frontiers
            .iter()
            .any(|frontier| frontier.less_equal(entry.key()))
        {
            return None;
        }
        Some(entry.remove())
    }
}

/// Format text for the status line trait
pub trait FormatText<'a, T: Timestamp + TotalOrder + std::hash::Hash> {
    /// Format text stream operation
    fn format_text(
        &self,
        configuration_stream: &ConfigurationStream<'a, T>,
        font_renderer: FontRenderer,
        key: usize,
    ) -> ImagePosStream<'a, T>;
}

impl<'a, T: Timestamp + TotalOrder + std::hash::Hash> FormatText<'a, T>
    for Stream<'a, T, Vec<Vec<String>>>
{
    fn format_text(
        &self,
        configuration_stream: &ConfigurationStream<'a, T>,
        mut font_renderer: FontRenderer,
        key: usize,
    ) -> ImagePosStream<'a, T> {
        let mut configuration_stash = HashMap::new();
        let mut text_stash = HashMap::new();
        let mut current_screen_dimension = None;
        let mut current_font_size = None;
        let mut current_font_canvas_vstretch = None;
        let mut current_text = None;
        self.clone().binary_frontier(
            configuration_stream.clone(),
            Pipeline,
            Pipeline,
            "Format text",
            |_capability, _info| {
                let mut pending = Pending::default();
                move |(in1, frontier1), (in2, frontier2), out| {
                    let _t = Timer::new(|e| debug!("Render font op {}ms", e.as_millis()));
                    in1.for_each(|time, data| {
                        // Messages without a time make no progress claims and have nowhere to go.
                        let Some(time) = time.retain_least(0) else {
                            return;
                        };
                        for text in data.drain(..) {
                            text_stash.insert(time.time().clone(), text);
                        }
                        pending.notify_at(time);
                    });
                    in2.for_each(|time, data| {
                        let Some(time) = time.retain_least(0) else {
                            return;
                        };
                        for configuration in data.drain(..) {
                            configuration_stash
                                .entry(time.time().clone())
                                .or_insert_with(Vec::new)
                                .push(configuration);
                        }
                        pending.notify_at(time);
                    });
                    while let Some(time) = pending.next(&[frontier1, frontier2]) {
                        if let Some(configurations) = configuration_stash.remove(time.time()) {
                            for configuration in configurations {
                                match configuration {
                                    Configuration::FontSize(font_size) => {
                                        current_font_size = Some(font_size)
                                    }
                                    Configuration::FontCanvasVStretch(font_canvas_vstretch) => {
                                        current_font_canvas_vstretch = Some(font_canvas_vstretch)
                                    }
                                    Configuration::ScreenDimensions(width, height) => {
                                        current_screen_dimension = Some((width, height))
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if let Some(text) = text_stash.remove(time.time()) {
                            current_text = Some(text);
                        }
                        if let (
                            Some(text),
                            Some(dimension),
                            Some(font_size),
                            Some(font_canvas_vstretch),
                        ) = (
                            &current_text,
                            &current_screen_dimension,
                            current_font_size,
                            current_font_canvas_vstretch,
                        ) {
                            // font canvas height, factor controls vertical padding
                            let canvas_height = (font_size * font_canvas_vstretch).ceil() as u32;
                            let canvas_height = canvas_height.min(dimension.1);
                            // Keep the text for a later configuration with a visible canvas.
                            if dimension.0 == 0 || canvas_height == 0 {
                                continue;
                            }
                            let img = font_renderer
                                .render(
                                    text.iter().map(String::as_str),
                                    font_size,
                                    (dimension.0, canvas_height),
                                )
                                .unwrap();
                            out.session(&time).give((
                                key,
                                Vector::new(0, dimension.1 as i32 - canvas_height as i32),
                                Arc::new(img.into()),
                            ));
                        }
                    }
                }
            },
        )
    }
}

/// Resize an image to match its viewport size
pub trait ResizeImage<'a, T: Timestamp + TotalOrder + std::hash::Hash> {
    /// Resize an image
    fn resize_image(
        &self,
        configuration_stream: &ConfigurationStream<'a, T>,
        key: usize,
    ) -> ImagePosStream<'a, T>;
}

impl<'a, T: Timestamp + TotalOrder + std::hash::Hash> ResizeImage<'a, T> for ImageStream<'a, T> {
    fn resize_image(
        &self,
        configuration_stream: &ConfigurationStream<'a, T>,
        key: usize,
    ) -> ImagePosStream<'a, T> {
        let mut img_stash = HashMap::new();
        let mut configuration_stash = HashMap::new();
        let mut current_screen_size = None;
        let mut current_image = None;
        self.clone().binary_frontier(
            configuration_stream.clone(),
            Pipeline,
            Pipeline,
            "Resize image",
            |_capability, _info| {
                let mut pending = Pending::default();
                move |(in1, frontier1), (in2, frontier2), out| {
                    let _t = Timer::new(|e| debug!("Resize image op {}ms", e.as_millis()));
                    in1.for_each(|time, data| {
                        // Messages without a time make no progress claims and have nowhere to go.
                        let Some(time) = time.retain_least(0) else {
                            return;
                        };
                        for img in data.drain(..) {
                            img_stash.insert(time.time().clone(), img);
                        }
                        pending.notify_at(time);
                    });
                    in2.for_each(|time, data| {
                        let Some(time) = time.retain_least(0) else {
                            return;
                        };
                        for configuration in data.drain(..) {
                            configuration_stash
                                .entry(time.time().clone())
                                .or_insert_with(Vec::new)
                                .push(configuration);
                        }
                        pending.notify_at(time);
                    });
                    while let Some(time) = pending.next(&[frontier1, frontier2]) {
                        if let Some(configurations) = configuration_stash.remove(time.time()) {
                            for configuration in configurations {
                                if let Configuration::ScreenDimensions(width, height) =
                                    configuration
                                {
                                    current_screen_size = Some((width, height))
                                }
                            }
                        }
                        if let Some(img) = img_stash.remove(time.time()) {
                            current_image = Some(img);
                        }
                        if let (Some(screen_size), Some(image)) =
                            (current_screen_size, current_image.as_ref())
                        {
                            // Resizing to zero still produces at least one pixel. Retain the
                            // image until a later configuration restores visible image space.
                            if screen_size.0 == 0 || screen_size.1 == 0 {
                                continue;
                            }
                            // Pictures prepared for this screen size pass through unchanged.
                            let resized = match image.resize_to_fit(screen_size.0, screen_size.1) {
                                Some(resized) => Arc::new(resized),
                                None => Arc::clone(image),
                            };
                            let x_offset = (screen_size.0 - resized.dimensions().0) / 2;
                            let y_offset = (screen_size.1 - resized.dimensions().1) / 2;
                            out.session(&time).give((
                                key,
                                Vector::new(x_offset as _, y_offset as _),
                                resized,
                            ));
                        }
                    }
                }
            },
        )
    }
}

#[cfg(test)]
mod tests;
