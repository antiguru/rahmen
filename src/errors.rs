//! Rahmen error handling

use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Error types within Rahmen
#[derive(std::fmt::Debug)]
pub enum RahmenError {
    /// Errors from handling EXIF data
    ExifError(exif::Error),
    /// Errors interacting with I/O
    IoError(std::io::Error),
    /// Errors from the image library
    ImageError(Arc<image::error::ImageError>),
    /// Pseudo-error to indicate a retry condition
    Retry,
    /// Pseudo-error to indocate program termination
    Terminate,
}

/// Result type for `RahmenError`
pub type RahmenResult<T> = Result<T, RahmenError>;

impl fmt::Display for RahmenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RahmenError::ExifError(err) => err.fmt(f),
            RahmenError::IoError(err) => err.fmt(f),
            RahmenError::ImageError(err) => err.fmt(f),
            RahmenError::Retry => write!(f, "Retry"),
            RahmenError::Terminate => write!(f, "Terminate"),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::ExifError(err) => err.source(),
            RahmenError::IoError(err) => err.source(),
            RahmenError::ImageError(err) => err.source(),
            RahmenError::Retry => None,
            RahmenError::Terminate => None,
        }
    }
}

impl From<exif::Error> for RahmenError {
    fn from(err: exif::Error) -> Self {
        RahmenError::ExifError(err)
    }
}

impl From<std::io::Error> for RahmenError {
    fn from(err: std::io::Error) -> Self {
        RahmenError::IoError(err)
    }
}

impl From<image::error::ImageError> for RahmenError {
    fn from(err: image::error::ImageError) -> Self {
        RahmenError::ImageError(Arc::new(err))
    }
}
