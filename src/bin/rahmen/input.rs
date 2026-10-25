use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use log::info;
use rahmen::errors::RahmenResult;
use rahmen::provider::Provider;
use rahmen::provider_list::ListProvider;

pub(super) fn select(
    input: &str,
    stdin: impl BufRead + 'static,
) -> RahmenResult<Box<dyn Provider<PathBuf>>> {
    // box is used bec of dynamic typing for provider
    let provider: Box<dyn Provider<_>> = if input.eq("-") {
        info!("Reading from stdin");
        Box::new(ListProvider::new(stdin))
    } else if let Ok(file) = File::open(input) {
        let reader = image::ImageReader::new(BufReader::new(file)).with_guessed_format()?;
        if reader.format().is_some() {
            info!("Reading single image {}", input);
            Box::new(SingleImage(Some(PathBuf::from(input))))
        } else {
            info!("Reading from file");
            Box::new(ListProvider::new(reader.into_inner()))
        }
    } else {
        info!("Reading from pattern {}", input);
        Box::new(rahmen::provider_glob::create(input)?)
    };
    Ok(provider)
}

struct SingleImage(Option<PathBuf>);

impl Provider<PathBuf> for SingleImage {
    fn next_image(&mut self) -> RahmenResult<Option<PathBuf>> {
        Ok(self.0.take())
    }
}

#[cfg(test)]
mod tests;
