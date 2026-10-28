use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, RgbImage, RgbaImage};

use crate::errors::{RahmenError, RahmenResult};
use crate::picture::{Picture, PixelFormat};

/// Display properties to prepare loaded images for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadTarget {
    /// Pixel format to decode JPEG images into, if the display has one
    pub format: Option<PixelFormat>,
    /// Size to fit images into, as `(width, height)`
    pub size: (u32, u32),
}

fn budget_error() -> RahmenError {
    image::ImageError::Limits(image::error::LimitError::from_kind(
        image::error::LimitErrorKind::InsufficientMemory,
    ))
    .into()
}

fn pixel_bytes(width: usize, height: usize, channels: usize) -> Option<usize> {
    width.checked_mul(height)?.checked_mul(channels)
}

fn check_budget(
    width: usize,
    height: usize,
    channels: usize,
    budget: Option<usize>,
) -> RahmenResult<()> {
    let bytes = pixel_bytes(width, height, channels).ok_or_else(budget_error)?;
    if budget.is_some_and(|budget| bytes > budget) {
        return Err(budget_error());
    }
    Ok(())
}

/// Decode a JPEG image, into `native` format if given, or else into RGB.
fn load_jpeg<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
    native: Option<(PixelFormat, mozjpeg::ColorSpace)>,
) -> RahmenResult<Picture> {
    let channels = if native.is_some() { 4 } else { 3 };
    let mut d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_path(&path)?;

    if let Some(budget) = max_size {
        // The decoder rounds each scaled dimension up to the next whole pixel.
        let scale = (1_u8..=8)
            .rev()
            .find(|&scale| {
                let dimensions = d
                    .width()
                    .checked_mul(usize::from(scale))
                    .zip(d.height().checked_mul(usize::from(scale)));
                dimensions
                    .and_then(|(width, height)| {
                        pixel_bytes(width.div_ceil(8), height.div_ceil(8), channels)
                    })
                    .is_some_and(|bytes| bytes <= budget)
            })
            .ok_or_else(budget_error)?;
        d.scale(scale);
    }
    let color_space = native.map_or(mozjpeg::ColorSpace::JCS_EXT_RGB, |(_, space)| space);
    let mut decompress_started = d.to_colorspace(color_space)?;
    let width = decompress_started.width();
    let height = decompress_started.height();
    check_budget(width, height, channels, max_size)?;
    let width = u32::try_from(width).map_err(|_| budget_error())?;
    let height = u32::try_from(height).map_err(|_| budget_error())?;
    // mozjpeg yields tightly packed scanlines, which map directly onto an image buffer.
    let pixels: Vec<u8> = decompress_started.read_scanlines::<u8>()?;
    let picture = match native {
        Some((format, _)) => {
            RgbaImage::from_raw(width, height, pixels).map(|image| Picture::Native(format, image))
        }
        None => RgbImage::from_raw(width, height, pixels)
            .map(|image| Picture::Image(DynamicImage::ImageRgb8(image))),
    };
    picture.ok_or_else(|| {
        error!("Failed to decode image: {:?}", path.as_ref());
        RahmenError::Retry
    })
}

/// Load an image from a path
pub fn load_image_from_path<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
) -> RahmenResult<DynamicImage> {
    match load_picture(path, max_size, None)? {
        Picture::Image(image) => Ok(image),
        picture => Ok(DynamicImage::ImageRgb8(picture.to_rgb8())),
    }
}

/// Load an image from a path and prepare it for display on `target`: decode JPEG images into the
/// target's pixel format where the decoder supports it, and fit the image into the target's size.
pub fn load_picture<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
    target: Option<LoadTarget>,
) -> RahmenResult<Picture> {
    let _t = crate::Timer::new(|e| debug!("Loading {}ms", e.as_millis()));
    info!("Loading {:?}", path.as_ref());
    let native = target
        .and_then(|target| target.format)
        .and_then(|format| Some((format, format.jpeg_color_space()?)));
    let picture = match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path, max_size, native)?,
        format => load_other(path, format, max_size)?.into(),
    };
    Ok(match target {
        Some(LoadTarget {
            size: (width, height),
            ..
        }) if width > 0 && height > 0 => {
            let _t = crate::Timer::new(|e| debug!("Resizing {}ms", e.as_millis()));
            picture.resize_to_fit(width, height).unwrap_or(picture)
        }
        _ => picture,
    })
}

/// Decode an image in a format other than JPEG.
fn load_other<P: AsRef<Path>>(
    path: P,
    format: image::ImageFormat,
    max_size: Option<usize>,
) -> RahmenResult<DynamicImage> {
    let mut reader =
        image::ImageReader::with_format(BufReader::new(std::fs::File::open(&path)?), format);
    let Some(budget) = max_size else {
        return reader.decode().map_err(Into::into);
    };
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(u64::try_from(budget).map_err(|_| budget_error())?);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    check_budget(
        usize::try_from(width).map_err(|_| budget_error())?,
        usize::try_from(height).map_err(|_| budget_error())?,
        usize::from(decoder.color_type().bytes_per_pixel()),
        Some(budget),
    )?;
    DynamicImage::from_decoder(decoder).map_err(Into::into)
}

#[cfg(test)]
mod tests;
