//! Provide a lost of files from a glob pattern

use crate::errors::RahmenResult;
use crate::provider::Provider;
use glob::glob;
use std::path::PathBuf;

/// Provider emitting paths from a glob pattern
#[derive(Debug)]
pub struct GlobProvider<I: Iterator<Item = PathBuf>> {
    path_iter: I,
}

/// Create a new `GlobProvider`
pub fn create(pattern: &str) -> RahmenResult<GlobProvider<impl Iterator<Item = PathBuf>>> {
    Ok(GlobProvider {
        path_iter: glob(pattern)
            .expect("Incorrect pattern")
            .filter_map(Result::ok),
    })
}

impl<I: Iterator<Item = PathBuf>> Provider<PathBuf> for GlobProvider<I> {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        Ok(self.path_iter.next())
    }
}
