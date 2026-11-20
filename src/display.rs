//! Functionality to render images on a display

use crate::Vector;
use crate::errors::RahmenResult;
use crate::picture::{Picture, PixelFormat};

/// Trait describing the interface to display an image
pub trait Display {
    /// Reveal an image to the user
    fn render(&mut self, key: usize, anchor: Vector, picture: &Picture) -> RahmenResult<()>;

    /// Reveal an image to the user
    fn blank(&mut self, key: usize, anchor: Vector, size: Vector) -> RahmenResult<()>;

    /// Update the image content. This would be a good opportunity to reveal any render/blank
    /// operations to the user.
    fn update(&mut self) -> RahmenResult<()>;

    /// Return the dimensions of the display as `(width, height)`
    fn dimensions(&self) -> (u32, u32);

    /// The display's native pixel format, if it has one. Pictures in this format can be shown
    /// without conversion.
    fn pixel_format(&self) -> Option<PixelFormat> {
        None
    }
}
