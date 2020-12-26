use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, Pixel};

use crate::errors::{RahmenError, RahmenResult};
use std::fmt::Display;

pub trait Provider<D> {
    fn next_image(&mut self) -> RahmenResult<D>;
}

impl<D> Provider<D> for Box<dyn Provider<D>> {
    fn next_image(&mut self) -> RahmenResult<D> {
        (**self).next_image()
    }
}

pub trait ToRahmenError<T> {
    fn map_to_rahmen_error(self, err: RahmenError) -> RahmenResult<T>;
}

impl<T, E: Display> ToRahmenError<T> for Result<T, E> {
    fn map_to_rahmen_error(self, err: RahmenError) -> RahmenResult<T> {
        self.map_err(|e| {
            eprintln!("Coercing {} to {}", e, err);
            err
        })
    }
}

fn load_jpeg<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
        .from_path(&path)
        .map_to_rahmen_error(RahmenError::Retry)?;
    let mut img = DynamicImage::new_bgra8(d.width() as _, d.height() as _);
    let height = d.height();
    let buffer: Option<Vec<[u8; 4]>> = d
        .to_colorspace(mozjpeg::ColorSpace::JCS_EXT_BGRA)
        .map_to_rahmen_error(RahmenError::Retry)?
        .read_scanlines();
    let rgba_img = img.as_mut_bgra8().unwrap();
    if let Some(buffer) = buffer {
        for (row, row_buffer) in buffer.chunks(buffer.len() / height).enumerate() {
            for (col, pixel) in row_buffer.iter().enumerate() {
                *rgba_img.get_pixel_mut(col as _, row as _) = *image::Bgra::from_slice(pixel);
            }
        }
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Retry)
    }
}

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| println!("Loading {}ms", e.as_millis()));
    println!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path).map_to_rahmen_error(RahmenError::Retry)? {
        image::ImageFormat::Jpeg => load_jpeg(path),
        format => Ok(image::io::Reader::with_format(
            BufReader::new(std::fs::File::open(&path).map_to_rahmen_error(RahmenError::Retry)?),
            format,
        )
        .decode()
        .map_to_rahmen_error(RahmenError::Retry)?),
    }
}
