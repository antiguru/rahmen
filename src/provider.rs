use std::io::BufReader;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::{DynamicImage, Pixel};

use crate::errors::{ProviderError, RahmenError, RahmenResult};

pub trait Provider<D> {
    fn next_image(&mut self) -> RahmenResult<D>;
}

impl<D> Provider<D> for Box<dyn Provider<D>> {
    fn next_image(&mut self) -> RahmenResult<D> {
        (**self).next_image()
    }
}

pub struct RateLimitingProvider<D, P: Provider<D>> {
    provider: P,
    interval: Duration,
    last_updated: Instant,
    next_image: Option<RahmenResult<D>>,
}

impl<D, P: Provider<D>> RateLimitingProvider<D, P> {
    pub fn new(provider: P, interval: Duration) -> Self {
        Self {
            provider,
            interval,
            last_updated: Instant::now() - interval,
            next_image: None,
        }
    }
}

impl<D, P: Provider<D>> Provider<D> for RateLimitingProvider<D, P> {
    fn next_image(&mut self) -> RahmenResult<D> {
        if self.next_image.is_none() {
            self.next_image = Some(self.provider.next_image());
        }

        if self.last_updated + self.interval < Instant::now() {
            self.last_updated = Instant::now();
            self.next_image.take().unwrap()
        } else {
            Err(RahmenError::Provider(ProviderError::Idle))
        }
    }
}

pub struct RetryProvider<D, P: Provider<D>> {
    provider: P,
    _phantom_data: PhantomData<D>,
}

impl<D, P: Provider<D>> RetryProvider<D, P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            _phantom_data: PhantomData,
        }
    }
}

impl<D, P: Provider<D>> Provider<D> for RetryProvider<D, P> {
    fn next_image(&mut self) -> RahmenResult<D> {
        loop {
            match self.provider.next_image() {
                Err(RahmenError::Provider(ProviderError::Retry)) => {}
                res => return res,
            }
        }
    }
}

fn load_jpeg<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
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
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Provider(ProviderError::Retry))
    }
}

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| println!("Loading {}ms", e.as_millis()));
    println!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path),
        format => Ok(image::io::Reader::with_format(
            BufReader::new(std::fs::File::open(&path)?),
            format,
        )
        .decode()?),
    }
}

pub trait PathToImageProvider<P: Provider<PathBuf>> {
    fn path_to_image(self) -> PathToImageProviderImpl<P>;
}

pub struct PathToImageProviderImpl<P: Provider<PathBuf>> {
    provider: P,
}

impl<P: Provider<PathBuf>> PathToImageProvider<P> for P {
    fn path_to_image(self) -> PathToImageProviderImpl<P> {
        PathToImageProviderImpl { provider: self }
    }
}

impl<P: Provider<PathBuf>> Provider<DynamicImage> for PathToImageProviderImpl<P> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        match self.provider.next_image() {
            Ok(path) => load_image_from_path(&path).map_err(|e| {
                eprintln!("Failed to read image {:?}: {}", path.as_os_str(), e);
                RahmenError::Provider(ProviderError::Retry)
            }),
            Err(e) => Err(e),
        }
    }
}
