//! Images on their way to a display, optionally already in the display's pixel layout

use crate::errors::{RahmenError, RahmenResult};
use framebuffer::{Bitfield, VarScreeninfo};
use image::{DynamicImage, RgbImage, RgbaImage};

/// Byte positions of the color channels within a 32-bit pixel, such as a framebuffer's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    red: usize,
    green: usize,
    blue: usize,
    alpha: Option<usize>,
}

fn invalid_format() -> RahmenError {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Unsupported framebuffer pixel format",
    )
    .into()
}

/// Map a channel's bit field to its byte position within a 32-bit pixel. Only byte-aligned
/// 8-bit channels are supported. Returns `None` for an absent channel.
fn channel_byte(field: &Bitfield) -> RahmenResult<Option<usize>> {
    if field.length == 0 {
        return Ok(None);
    }
    if field.length != 8
        || !field.offset.is_multiple_of(8)
        || field.offset >= 32
        || field.msb_right != 0
    {
        return Err(invalid_format());
    }
    // Bit offsets refer to the pixel as a native-endian 32-bit value.
    let byte = (field.offset / 8) as usize;
    Ok(Some(if cfg!(target_endian = "little") {
        byte
    } else {
        3 - byte
    }))
}

impl PixelFormat {
    /// Determine the pixel format of a 32 bits per pixel framebuffer.
    pub fn from_var_screen_info(var: &VarScreeninfo) -> RahmenResult<Self> {
        if var.bits_per_pixel != 32 {
            return Err(invalid_format());
        }
        let red = channel_byte(&var.red)?.ok_or_else(invalid_format)?;
        let green = channel_byte(&var.green)?.ok_or_else(invalid_format)?;
        let blue = channel_byte(&var.blue)?.ok_or_else(invalid_format)?;
        let alpha = channel_byte(&var.transp)?;
        let mut bytes = [Some(red), Some(green), Some(blue), alpha];
        bytes.sort();
        if bytes.windows(2).any(|w| w[0].is_some() && w[0] == w[1]) {
            return Err(invalid_format());
        }
        Ok(Self {
            red,
            green,
            blue,
            alpha,
        })
    }

    /// An opaque black pixel. The byte not holding a color is `0xFF`, matching what the JPEG
    /// decoder writes there.
    pub fn black(&self) -> [u8; 4] {
        let mut pixel = [u8::MAX; 4];
        pixel[self.red] = 0;
        pixel[self.green] = 0;
        pixel[self.blue] = 0;
        pixel
    }

    /// Encode an RGB color as a pixel in this format.
    pub fn encode(&self, [red, green, blue]: [u8; 3]) -> [u8; 4] {
        let mut pixel = [u8::MAX; 4];
        pixel[self.red] = red;
        pixel[self.green] = green;
        pixel[self.blue] = blue;
        pixel
    }

    /// Decode a pixel in this format to an RGB color.
    pub fn decode(&self, pixel: [u8; 4]) -> [u8; 3] {
        [pixel[self.red], pixel[self.green], pixel[self.blue]]
    }

    /// The JPEG decoder output color space producing pixels in this format, if there is one.
    pub fn jpeg_color_space(&self) -> Option<mozjpeg::ColorSpace> {
        use mozjpeg::ColorSpace::*;
        // The `*A` spaces set the fourth byte to 0xFF, while the `*X` spaces leave it undefined.
        match (self.red, self.green, self.blue) {
            (0, 1, 2) => Some(JCS_EXT_RGBA),
            (2, 1, 0) => Some(JCS_EXT_BGRA),
            (1, 2, 3) => Some(JCS_EXT_ARGB),
            (3, 2, 1) => Some(JCS_EXT_ABGR),
            _ => None,
        }
    }
}

/// The largest size with the aspect ratio of `size` that fits `bounds`, rounded like
/// [`DynamicImage::resize`].
fn fit_dimensions(size: (u32, u32), bounds: (u32, u32)) -> (u32, u32) {
    let ratio = f64::min(
        f64::from(bounds.0) / f64::from(size.0),
        f64::from(bounds.1) / f64::from(size.1),
    );
    let scale = |length: u32| ((f64::from(length) * ratio).round() as u32).max(1);
    (scale(size.0), scale(size.1))
}

/// An image to show on a display.
#[derive(Debug, Clone, PartialEq)]
pub enum Picture {
    /// An image with ordinary color channels
    Image(DynamicImage),
    /// Four bytes per pixel in the given format. The buffer's channels are not RGBA, it only
    /// serves as storage.
    Native(PixelFormat, RgbaImage),
}

impl From<DynamicImage> for Picture {
    fn from(image: DynamicImage) -> Self {
        Picture::Image(image)
    }
}

impl Picture {
    /// Return the dimensions as `(width, height)`
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Picture::Image(image) => (image.width(), image.height()),
            Picture::Native(_, image) => image.dimensions(),
        }
    }

    /// Scale the picture to the largest size fitting `width` and `height`, preserving its aspect
    /// ratio. Returns `None` if the picture already touches both bounds or one bound while
    /// fitting the other.
    pub fn resize_to_fit(&self, width: u32, height: u32) -> Option<Picture> {
        let (w, h) = self.dimensions();
        if (w == width && h <= height) || (h == height && w <= width) {
            return None;
        }
        let filter = image::imageops::FilterType::Triangle;
        Some(match self {
            Picture::Image(image) => Picture::Image(image.resize(width, height, filter)),
            // The filter treats channels independently, so their order does not matter.
            Picture::Native(format, image) => {
                let (w, h) = fit_dimensions((w, h), (width, height));
                Picture::Native(*format, image::imageops::resize(image, w, h, filter))
            }
        })
    }

    /// Convert to an RGB image.
    pub fn to_rgb8(&self) -> RgbImage {
        match self {
            Picture::Image(image) => image.to_rgb8(),
            Picture::Native(format, image) => {
                RgbImage::from_fn(image.width(), image.height(), |x, y| {
                    image::Rgb(format.decode(image.get_pixel(x, y).0))
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
