//! Rahmen library

#![forbid(unsafe_code)]
#![warn(
    anonymous_parameters,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_qualifications,
    variant_size_differences
)]

use std::time::{Duration, Instant};

pub mod dataflow;
pub mod display;
#[cfg(feature = "fltk")]
pub mod display_fltk;
pub mod display_framebuffer;
pub mod errors;
pub mod font;
pub mod provider;
pub mod provider_glob;
pub mod provider_list;

/// A timer to track the duration of a code span until it is dropped
#[derive(Debug)]
pub struct Timer<F: Fn(Duration)> {
    start: Instant,
    f: F,
}

impl<F: Fn(Duration)> Timer<F> {
    /// Construct a new timer with a callback. The callback will receive the duration since creation
    pub fn new(f: F) -> Self {
        Self {
            start: Instant::now(),
            f,
        }
    }
}

impl<F: Fn(Duration)> Drop for Timer<F> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        if elapsed >= Duration::from_millis(1) {
            (self.f)(elapsed)
        }
    }
}
