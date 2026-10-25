use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, RgbImage};

use crate::errors::{RahmenError, RahmenResult};

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

fn load_jpeg<P: AsRef<Path>>(path: P, max_size: Option<usize>) -> RahmenResult<DynamicImage> {
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
                        pixel_bytes(width.div_ceil(8), height.div_ceil(8), 3)
                    })
                    .is_some_and(|bytes| bytes <= budget)
            })
            .ok_or_else(budget_error)?;
        d.scale(scale);
    }
    let mut decompress_started = d.to_colorspace(mozjpeg::ColorSpace::JCS_EXT_RGB)?;
    let width = decompress_started.width();
    let height = decompress_started.height();
    check_budget(width, height, 3, max_size)?;
    let width = u32::try_from(width).map_err(|_| budget_error())?;
    let height = u32::try_from(height).map_err(|_| budget_error())?;
    // mozjpeg yields tightly packed RGB scanlines, which map directly onto an `RgbImage`.
    let pixels: Vec<u8> = decompress_started.read_scanlines::<u8>()?;
    RgbImage::from_raw(width, height, pixels)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| {
            error!("Failed to decode image: {:?}", path.as_ref());
            RahmenError::Retry
        })
}

/// Load an image from a path
pub fn load_image_from_path<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| debug!("Loading {}ms", e.as_millis()));
    info!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path, max_size),
        format => {
            let mut reader = image::ImageReader::with_format(
                BufReader::new(std::fs::File::open(&path)?),
                format,
            );
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
    }
}

#[cfg(test)]
mod tests;
