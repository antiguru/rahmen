//! Dataflow operators to handle images are required for Rahmen

use std::collections::HashMap;
use std::sync::Arc;

use crate::font::FontRenderer;
use crate::Timer;
use image::{DynamicImage, GenericImage, GenericImageView, Pixel};
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::Operator;
use timely::dataflow::{Scope, Stream};

/// A stream of images
pub type ImageStream<S> = Stream<S, Arc<DynamicImage>>;

/// A keyed stream of offset and image
pub type ImagePosStream<S> = Stream<S, (usize, (u32, u32), Arc<DynamicImage>)>;

/// A configuration stream
pub type ConfigurationStream<S> = Stream<S, Configuration>;

/// A configuration element to be passed in a configuration stream
#[derive(Debug, Clone, Copy)]
pub enum Configuration {
    /// Update the font size for the status line
    FontSize(f32),
    /// factor by which font canvas is higher than font
    FontCanvasVStretch(f32),
    /// show time in status bar or don't
    ShowTime(bool),
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

impl<S: Scope> FormatText<S> for Stream<S, String> {
    fn format_text(
        &self,
        configuration_stream: &ConfigurationStream<S>,
        font_renderer: FontRenderer,
        key: usize,
    ) -> ImagePosStream<S> {
        let mut configuration_stash = HashMap::new();
        let mut text_stash = HashMap::new();
        let mut current_screen_dimension = None;
        let mut current_font_size = None;
        let mut current_font_canvas_vstretch = None;
        let mut current_show_time = None;
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
                                Configuration::ShowTime(show_time) => {
                                    current_show_time = Some(show_time)
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
                    if current_text.is_some()
                        && current_screen_dimension.is_some()
                        && current_font_size.is_some()
                        && current_font_canvas_vstretch.is_some()
                    {
                        let font_size = current_font_size.unwrap();
                        // font canvas height, factor controls vertical padding
                        let canvas_height = font_size * current_font_canvas_vstretch.unwrap();
                        let dimension = current_screen_dimension.as_ref().unwrap();
                        let mut img = DynamicImage::new_luma8(dimension.0, canvas_height as _);
                        font_renderer
                            .render(
                                current_text.as_ref().unwrap(),
                                font_size,
                                (dimension.0, canvas_height as _),
                                |x, y, pixel| {
                                    img.put_pixel(x as _, y as _, pixel.to_rgba());
                                    Ok(())
                                },
                            )
                            .unwrap();
                        out.session(&time).give((
                            key,
                            (0, dimension.1 - canvas_height as u32),
                            Arc::new(img),
                        ));
                    }
                });
            },
        )
    }
}

/// Compose a set of images identified by a key with offsets into a final image
pub trait ComposeImage<S: Scope> {
    /// Compose the images
    fn compose_image(&self, configuration_stream: &ConfigurationStream<S>) -> ImageStream<S>;
}

impl<S: Scope> ComposeImage<S> for ImagePosStream<S> {
    fn compose_image(&self, configuration_stream: &ConfigurationStream<S>) -> ImageStream<S> {
        let mut buffer1 = vec![];
        let mut buffer2 = vec![];
        let mut img_stash = HashMap::new();
        let mut configuration_stash = HashMap::new();
        let mut current_screen_size = None;
        let mut current_image = HashMap::new();
        self.binary_notify(
            &configuration_stream,
            Pipeline,
            Pipeline,
            "Compose image",
            None,
            move |in1, in2, out, not| {
                let _t = Timer::new(|e| println!("Compose image op {}ms", e.as_millis()));
                in1.for_each(|time, data| {
                    data.swap(&mut buffer1);
                    for img in buffer1.drain(..) {
                        img_stash
                            .entry(time.time().clone())
                            .or_insert_with(Vec::new)
                            .push(img);
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
                    if let Some(imgs) = img_stash.remove(time.time()) {
                        for img in imgs {
                            current_image.insert(img.0, img);
                        }
                    }
                    if let Some(current_screen_size) = current_screen_size {
                        let mut output_image =
                            DynamicImage::new_bgr8(current_screen_size.0, current_screen_size.1);
                        println!("current screen size: {:?}", current_screen_size);
                        for (_key, (x_offset, y_offset), img) in current_image.values() {
                            println!(
                                " Key:{} -> xy:({}, {}) + img:{:?}",
                                _key,
                                x_offset,
                                y_offset,
                                img.as_ref().dimensions()
                            );
                            output_image
                                .copy_from(img.as_ref(), *x_offset, *y_offset)
                                .unwrap();
                        }
                        out.session(&time).give(Arc::new(output_image));
                    }
                })
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
                    if current_screen_size.is_some() && current_image.is_some() {
                        let current_screen_size = current_screen_size.unwrap();
                        let current_image = current_image.as_ref().unwrap();
                        let resized = current_image.resize(
                            current_screen_size.0,
                            current_screen_size.1,
                            image::imageops::FilterType::Triangle,
                        );
                        let x_offset = (current_screen_size.0 - resized.dimensions().0) / 2;
                        let y_offset = (current_screen_size.1 - resized.dimensions().1) / 2;
                        out.session(&time)
                            .give((key, (x_offset, y_offset), Arc::new(resized)));
                    }
                })
            },
        )
    }
}
