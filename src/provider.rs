//! Utilities to provide images, and other abstractions

use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, Pixel};
use rexiv2::Metadata;

use crate::errors::{RahmenError, RahmenResult};

/// Provider trait to produce images, or other types
pub trait Provider<D> {
    /// Obtain the next element.
    /// Error -> Terminate
    /// Ok(Some(T)) -> Process T
    /// Ok(None) -> Exhausted
    fn next_image(&mut self) -> RahmenResult<Option<D>>;
}

impl<D> Provider<D> for Box<dyn Provider<D>> {
    fn next_image(&mut self) -> RahmenResult<Option<D>> {
        (**self).next_image()
    }
}

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
    let mut decompress_started = d.to_colorspace(mozjpeg::ColorSpace::JCS_EXT_BGR)?;
    let height = decompress_started.height();
    let mut img = DynamicImage::new_bgr8(decompress_started.width() as _, height as _);
    let buffer: Option<Vec<[u8; 3]>> = decompress_started.read_scanlines();
    let rgb_img = img.as_mut_bgr8().unwrap();
    if let Some(buffer) = buffer {
        for (row, row_buffer) in buffer.chunks(buffer.len() / height).enumerate() {
            for (col, pixel) in row_buffer.iter().enumerate() {
                *rgb_img.get_pixel_mut(col as _, row as _) = *image::Bgr::from_slice(pixel);
            }
        }
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Retry.into())
    }
}

/// Load an image from a path
pub fn load_image_from_path<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| println!("Loading {}ms", e.as_millis()));
    println!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path, max_size),
        format => {
            image::io::Reader::with_format(BufReader::new(std::fs::File::open(&path)?), format)
                .decode()
                .map_err(Into::into)
        }
    }
}

const FIELD_LOOKUP_TABLE: &[&[&str]] = &[
    &[
        "Iptc.Application2.Sublocation",
        "Iptc.Application2.City",
        "Iptc.Application2.ProvinceState",
        "Iptc.Application2.CountryName",
        "Iptc.Application2.CountryCode",
    ],
    &["Iptc.Application2.DigitizationDate"],
    &["Xmp.dc.creator"],
];

/// Format the metadata tags from an image to show a status line
pub fn format_exif<P: AsRef<std::ffi::OsStr>>(path: P) -> RahmenResult<String> {
    let metadata = Metadata::new_from_path(path)?;

    let mut result = vec![];
    for lookup in FIELD_LOOKUP_TABLE {
        if let Some(Some(text)) = lookup
            .iter()
            .filter(|f| metadata.has_tag(f))
            .map(|f| metadata.get_tag_interpreted_string(*f).ok())
            .filter(Option::is_some)
            .next()
        {
            result.push(text)
        }
    }
    Ok(result.join(" "))
}
