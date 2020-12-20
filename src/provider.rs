use std::time::{Duration, Instant};

use image::DynamicImage;

use crate::errors::{ProviderError, RahmenError, RahmenResult};

pub trait Provider {
    fn next_image(&mut self) -> RahmenResult<DynamicImage>;
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
