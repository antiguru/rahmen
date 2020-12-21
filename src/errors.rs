use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RahmenError {
    ImageError(image::error::ImageError),
    IoError(std::io::Error),
    Provider(ProviderError),
}

#[derive(Debug)]
pub enum ProviderError {
    Eof,
    Idle,
    Retry,
}

impl fmt::Display for RahmenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RahmenError::ImageError(err) => err.fmt(f),
            RahmenError::IoError(err) => err.fmt(f),
            RahmenError::Provider(err) => err.fmt(f),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::ImageError(err) => err.source(),
            RahmenError::IoError(err) => err.source(),
            RahmenError::Provider(err) => err.source(),
        }
    }
}

impl From<std::io::Error> for RahmenError {
    fn from(e: std::io::Error) -> Self {
        RahmenError::IoError(e)
    }
}

impl From<image::error::ImageError> for RahmenError {
    fn from(e: image::error::ImageError) -> Self {
        RahmenError::ImageError(e)
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            ProviderError::Idle => write!(f, "Provider idle"),
            ProviderError::Eof => write!(f, "Provider exhausted (Eof)"),
            ProviderError::Retry => write!(f, "Retry"),
        }
    }
}

impl Error for ProviderError {}
pub type RahmenResult<T> = Result<T, RahmenError>;
