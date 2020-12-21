use crate::errors::{ProviderError, RahmenError, RahmenResult};
use crate::provider::{load_image_from_path, Provider};
use glob::glob;
use image::DynamicImage;
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

impl<I: Iterator<Item = PathBuf>> Provider for GlobProvider<I> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        self.path_iter
            .next()
            .ok_or(RahmenError::Provider(ProviderError::Eof))
            .and_then(load_image_from_path)
    }
}
