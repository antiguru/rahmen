use std::time::{Duration, Instant};

use image::{DynamicImage, Pixel};

use crate::errors::{ProviderError, RahmenError, RahmenResult};
use std::io::BufReader;
use std::path::Path;

pub trait Provider {
    fn next_image(&mut self) -> RahmenResult<DynamicImage>;
}

impl Provider for Box<dyn Provider> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        (**self).next_image()
    }
}

pub struct RateLimitingProvider<P: Provider> {
    provider: P,
    interval: Duration,
    last_updated: Instant,
}

impl<P: Provider> RateLimitingProvider<P> {
    pub fn new(provider: P, interval: Duration) -> Self {
        Self {
            provider,
            interval,
            last_updated: Instant::now() - interval,
        }
    }
}

impl<P: Provider> Provider for RateLimitingProvider<P> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        if self.last_updated + self.interval < Instant::now() {
            self.last_updated = Instant::now();
            self.provider.next_image()
        } else {
            Err(RahmenError::Provider(ProviderError::Idle))
        }
    }
}

pub struct ImageErrorToRetryProvider<P: Provider>(P);

impl<P: Provider> ImageErrorToRetryProvider<P> {
    pub fn new(provider: P) -> Self {
        Self(provider)
    }
}

impl<P: Provider> Provider for ImageErrorToRetryProvider<P> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        match self.0.next_image() {
            Err(RahmenError::ImageError(_)) => Err(RahmenError::Provider(ProviderError::Retry)),
            res => res,
        }
    }
}

pub struct RetryProvider<P: Provider>(P);

impl<P: Provider> RetryProvider<P> {
    pub fn new(provider: P) -> Self {
        Self(provider)
    }
}

impl<P: Provider> Provider for RetryProvider<P> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        loop {
            match self.0.next_image() {
                Err(RahmenError::Provider(ProviderError::Retry)) => {}
                res => return res,
            }
        }
    }
}

fn load_jpeg<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    println!("Loading {:?}", path.as_ref());
    let start = Instant::now();
    let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_path(&path)?;
    let mut img = DynamicImage::new_bgra8(d.width() as _, d.height() as _);
    let height = d.height();
    let buffer: Option<Vec<[u8; 4]>> = d
        .to_colorspace(mozjpeg::ColorSpace::JCS_EXT_BGRA)?
        .read_scanlines();
    let rgba_img = img.as_mut_bgra8().unwrap();
    if let Some(buffer) = buffer {
        for (row, row_buffer) in buffer.chunks(buffer.len() / height).enumerate() {
            for (col, pixel) in row_buffer.iter().enumerate() {
                *rgba_img.get_pixel_mut(col as _, row as _) = *image::Bgra::from_slice(pixel);
            }
        }
        println!("Loading took: {}ms", start.elapsed().as_millis());
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Provider(ProviderError::Retry))
    }
}

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    println!("Loading {:?}", path.as_ref());
    let start = Instant::now();
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path),
        format => Ok(image::io::Reader::with_format(
            BufReader::new(std::fs::File::open(&path)?),
            format,
        )
        .decode()?),
    }
}
