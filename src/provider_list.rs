//! Provide a list of files from a file input

use crate::errors::RahmenResult;
use crate::provider::Provider;
use std::io::BufRead;
use std::path::PathBuf;

/// Provider to read paths line-by-line from a reader, which can be backed by an input stream or
/// file
#[derive(Debug)]
pub struct ListProvider<R: BufRead> {
    reader: R,
    buffer: String,
}

impl<R: BufRead> ListProvider<R> {
    /// Create a new `ListProvider`, passing in a reader
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: String::new(),
        }
    }
}
impl<R: BufRead> Provider<PathBuf> for ListProvider<R> {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        self.buffer.clear();
        if self.reader.read_line(&mut self.buffer)? == 0 {
            Ok(None)
        } else {
            let trimmed = &self.buffer.trim();
            Ok(Some(PathBuf::from(trimmed)))
        }
    }
}
