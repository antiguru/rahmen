use std::time::{Duration, Instant};

use image::{DynamicImage, ImageFormat};

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

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    Ok(image::io::Reader::with_format(
        BufReader::new(std::fs::File::open(&path)?),
        ImageFormat::from_path(&path)?,
    )
    .decode()?)
}
