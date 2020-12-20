use crate::errors::{ProviderError, RahmenError, RahmenResult};
use crate::provider::Provider;
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::PathBuf;

pub struct ListProvider<R>
where
    R: BufRead,
{
    reader: R,
    buffer: String,
}

fn load_image<R: Read + BufRead + Seek>(reader: R, format: image::ImageFormat) -> DynamicImage {
    let img = image::io::Reader::with_format(reader, format);
    let img = img.decode().expect("Failed to decode");
    println!("decoded img {:?}", img.dimensions());
    img
}

impl<R> ListProvider<R>
where
    R: BufRead,
{
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: String::new(),
        }
    }
}
impl<R: BufRead> Provider for ListProvider<R> {
    fn next_image(&mut self) -> RahmenResult<DynamicImage> {
        loop {
            self.buffer.clear();
            let length = self
                .reader
                .read_line(&mut self.buffer)
                .expect("Reading from file failed");
            if length == 0 {
                return Err(RahmenError::Provider(ProviderError::Eof));
            }
            let trimmed = &self.buffer.trim();
            println!("Reading {}", trimmed);
            let path = PathBuf::from(trimmed);
            match ImageFormat::from_path(&path) {
                Ok(format) => {
                    println!("format {:?}", format);
                    return Ok(load_image(
                        BufReader::new(std::fs::File::open(&path).expect("failed to open")),
                        format,
                    ));
                }
                Err(e @ image::error::ImageError::Unsupported(_)) => eprintln!("{}", e),
                _ => eprintln!("Unknown format: {}", trimmed),
            }
        }
    }
}
