//! Dataflow operators to handle images are required for Rahmen

use std::collections::HashMap;
use std::sync::Arc;

use crate::font::FontRenderer;
use crate::{Timer, Vector};
use image::{DynamicImage, GenericImageView};
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::Operator;
use timely::dataflow::{Scope, Stream};

/// A stream of images
pub type ImageStream<S> = Stream<S, Arc<DynamicImage>>;

/// A keyed stream of offset and image
pub type ImagePosStream<S> = Stream<S, (usize, Vector, Arc<DynamicImage>)>;

/// A configuration stream
pub type ConfigurationStream<S> = Stream<S, Configuration>;

/// A configuration element to be passed in a configuration stream
#[derive(Debug, Clone, Copy)]
pub enum Configuration {
    /// Update the font size for the status line
    FontSize(f32),
    /// factor by which font canvas is higher than font
    FontCanvasVStretch(f32),
    /// Update the screen dimensions
    ScreenDimensions(u32, u32),
    /// Show a new image
    Tick,
}

/// Format text for the status line trait
pub trait FormatText<S: Scope> {
    /// Format text stream operation
    fn format_text(
        &self,
        configuration_stream: &ConfigurationStream<S>,
        font_renderer: FontRenderer,
        key: usize,
    ) -> ImagePosStream<S>;
}

impl<S: Scope> FormatText<S> for Stream<S, Vec<String>> {
    fn format_text(
        &self,
        configuration_stream: &ConfigurationStream<S>,
        mut font_renderer: FontRenderer,
        key: usize,
    ) -> ImagePosStream<S> {
        let mut configuration_stash = HashMap::new();
        let mut text_stash = HashMap::new();
        let mut current_screen_dimension = None;
        let mut current_font_size = None;
        let mut current_font_canvas_vstretch = None;
        let mut current_text = None;
        let mut in_buffer1 = vec![];
        let mut in_buffer2 = vec![];
        self.binary_notify(
            configuration_stream,
            Pipeline,
            Pipeline,
            "Format text",
            None,
            move |in1, in2, out, not| {
                let _t = Timer::new(|e| println!("Render font op {}ms", e.as_millis()));
                in1.for_each(|time, data| {
                    data.swap(&mut in_buffer1);
                    for text in in_buffer1.drain(..) {
                        text_stash.insert(time.time().clone(), text);
                    }
                    not.notify_at(time.retain());
                });
                in2.for_each(|time, data| {
                    data.swap(&mut in_buffer2);
                    for configuration in in_buffer2.drain(..) {
                        configuration_stash
                            .entry(time.time().clone())
                            .or_insert_with(Vec::new)
                            .push(configuration);
                    }
                    not.notify_at(time.retain());
                });
                not.for_each(|time, _cnt, _not| {
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
                        let canvas_height = font_size * font_canvas_vstretch;
                        let img = font_renderer
                            .render(
                                text.iter().map(String::as_str),
                                font_size,
                                (dimension.0, canvas_height as _),
                            )
                            .unwrap();
                        out.session(&time).give((
                            key,
                            Vector::new(0, dimension.1 as i32 - canvas_height as i32),
                            Arc::new(img),
                        ));
                    }
                });
            },
        )
    }
}

/// Resize an image to match its viewport size
pub trait ResizeImage<S: Scope> {
    /// Resize an image
    fn resize_image(
        &self,
        configuration_stream: &ConfigurationStream<S>,
        key: usize,
    ) -> ImagePosStream<S>;
}

impl<S: Scope> ResizeImage<S> for ImageStream<S> {
    fn resize_image(
        &self,
        configuration_stream: &ConfigurationStream<S>,
        key: usize,
    ) -> ImagePosStream<S> {
        let mut buffer1 = vec![];
        let mut buffer2 = vec![];
        let mut img_stash = HashMap::new();
        let mut configuration_stash = HashMap::new();
        let mut current_screen_size = None;
        let mut current_image = None;
        self.binary_notify(
            &configuration_stream,
            Pipeline,
            Pipeline,
            "Resize image",
            None,
            move |in1, in2, out, not| {
                let _t = Timer::new(|e| println!("Resize image op {}ms", e.as_millis()));
                in1.for_each(|time, data| {
                    data.swap(&mut buffer1);
                    for img in buffer1.drain(..) {
                        img_stash.insert(time.time().clone(), img);
                    }
                    not.notify_at(time.retain());
                });
                in2.for_each(|time, data| {
                    data.swap(&mut buffer2);
                    for configuration in buffer2.drain(..) {
                        configuration_stash
                            .entry(time.time().clone())
                            .or_insert_with(Vec::new)
                            .push(configuration);
                    }
                    not.notify_at(time.retain());
                });
                not.for_each(|time, _cnt, _not| {
                    if let Some(configurations) = configuration_stash.remove(time.time()) {
                        for configuration in configurations {
                            if let Configuration::ScreenDimensions(width, height) = configuration {
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
                        let resized = image.resize(
                            screen_size.0,
                            screen_size.1,
                            image::imageops::FilterType::Triangle,
                        );
                        let x_offset = (screen_size.0 - resized.dimensions().0) / 2;
                        let y_offset = (screen_size.1 - resized.dimensions().1) / 2;
                        out.session(&time).give((
                            key,
                            Vector::new(x_offset as _, y_offset as _),
                            Arc::new(resized),
                        ));
                    }
                })
            },
        )
    }
}
