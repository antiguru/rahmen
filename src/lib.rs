extern crate glob;
extern crate image;
extern crate linuxfb;
extern crate memmap;
#[cfg(feature = "minifb")]
extern crate minifb;
extern crate mozjpeg;

use std::time::{Duration, Instant};

pub mod display;
pub mod display_framebuffer;
pub mod display_linuxfb;
#[cfg(feature = "minifb")]
pub mod display_minifb;
pub mod errors;
pub mod provider;
pub mod provider_glob;
pub mod provider_list;

pub(crate) struct Timer<F: Fn(Duration)> {
    start: Instant,
    f: F,
}

impl<F: Fn(Duration)> Timer<F> {
    pub(crate) fn new(f: F) -> Self {
        Self {
            start: Instant::now(),
            f,
        }
    }
}

impl<F: Fn(Duration)> Drop for Timer<F> {
    fn drop(&mut self) {
        (self.f)(self.start.elapsed())
    }
}
