use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, RgbImage};

use crate::errors::{RahmenError, RahmenResult};

fn load_jpeg<P: AsRef<Path>>(path: P, max_size: Option<usize>) -> RahmenResult<DynamicImage> {
    let mut d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_path(&path)?;

    if let Some(max_size) = max_size {
        let mut scale = 8;
        let ratio_to_max_size = max_size as f32 / (d.width() * d.height()) as f32;
        if ratio_to_max_size < 1. {
            scale = (ratio_to_max_size * 8.) as u8 + 1;
        }
        d.scale(scale);
    }
    let mut decompress_started = d.to_colorspace(mozjpeg::ColorSpace::JCS_EXT_RGB)?;
    let width = decompress_started.width();
    let height = decompress_started.height();
    // mozjpeg yields tightly packed RGB scanlines, which map directly onto an `RgbImage`.
    let pixels: Vec<u8> = decompress_started.read_scanlines::<u8>()?;
    RgbImage::from_raw(width as _, height as _, pixels)
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
            image::ImageReader::with_format(BufReader::new(std::fs::File::open(&path)?), format)
                .decode()
                .map_err(Into::into)
        }
    }
}

