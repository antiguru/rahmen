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
        info!("Reading from file");
        Box::new(ListProvider::new(BufReader::new(file)))
    } else {
        info!("Reading from pattern {}", input);
        Box::new(rahmen::provider_glob::create(input)?)
    };
    Ok(provider)
}

#[cfg(test)]
mod tests;
