use crate::errors::{RahmenError, RahmenResult};
use crate::provider::Provider;
use glob::glob;
use std::path::PathBuf;

pub struct GlobProvider<I: Iterator<Item = PathBuf>> {
    path_iter: I,
}

pub fn create(pattern: &str) -> RahmenResult<GlobProvider<impl Iterator<Item = PathBuf>>> {
    Ok(GlobProvider {
        path_iter: glob(pattern)
            .expect("Incorrect pattern")
            .filter_map(Result::ok),
    })
}

impl<I: Iterator<Item = PathBuf>> Provider<PathBuf> for GlobProvider<I> {
    fn next_image(&mut self) -> RahmenResult<PathBuf> {
        self.path_iter.next().ok_or(RahmenError::Terminate)
    }
}
