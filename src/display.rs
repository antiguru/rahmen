//! Functionality to render images on a display

use image::DynamicImage;

use crate::errors::RahmenResult;
use crate::Vector;

/// Trait describing the interface to display an image
pub trait Display {
    /// Reveal an image to the user
    fn render(&mut self, key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()>;

    /// Reveal an image to the user
    fn blank(&mut self, key: usize, anchor: Vector, size: Vector) -> RahmenResult<()>;

    /// Update the image content. This would be a good opportunity to reveal any render/blank
    /// operations to the user.
    fn update(&mut self) -> RahmenResult<()>;

    /// Return the dimensions of the display as `(width, height)`
    fn dimensions(&self) -> (u32, u32);
}
