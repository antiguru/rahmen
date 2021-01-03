//! Functionality to render images on a display

use image::DynamicImage;

use crate::errors::RahmenResult;

/// Trait describing the interface to display an image
pub trait Display {
    /// Reveal an image to the user
    fn render(&mut self, img: &DynamicImage) -> RahmenResult<()>;

    /// Return the dimensions of the display as `(width, height)`
    fn dimensions(&self) -> (u32, u32);
}
