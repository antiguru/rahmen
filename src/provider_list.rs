use crate::errors::{ProviderError, RahmenError, RahmenResult};
use crate::provider::{load_image_from_path, Provider};
use image::DynamicImage;
use std::io::BufRead;
use std::path::PathBuf;

pub struct ListProvider<R: BufRead> {
    reader: R,
    buffer: String,
}

impl<R: BufRead> ListProvider<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: String::new(),
        }
    }
}
impl<R: BufRead> Provider for ListProvider<R> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        self.buffer.clear();
        if self.reader.read_line(&mut self.buffer)? == 0 {
            Err(RahmenError::Provider(ProviderError::Eof))
        } else {
            let trimmed = &self.buffer.trim();
            println!("Reading {}", trimmed);
            load_image_from_path(&PathBuf::from(trimmed))
        }
    }
}
