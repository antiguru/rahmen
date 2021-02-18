//! Rahmen error handling

use std::error::Error;
use std::fmt;
use std::num::ParseFloatError;
use std::sync::Arc;

/// Error types within Rahmen
#[derive(std::fmt::Debug)]
pub enum RahmenError {
    /// unknown case for conversion
    CaseUnknown(String),
    /// Errors originating from config loading
    ConfigError(Arc<config::ConfigError>),
    /// Errors interacting with I/O
    IoError(std::io::Error),
    /// Errors from the image library
    ImageError(Arc<image::error::ImageError>),
    /// Parsing a float failed
    ParseFloatError(ParseFloatError),
    /// Errors form the Python interpreter
    PythonError(pyo3::prelude::PyErr),
    /// An error originating from regex processing
    RegexError(regex::Error),
    /// Pseudo-error to indicate a retry condition
    Retry,
    /// Errors from rexiv2
    Rexiv2Error(rexiv2::Rexiv2Error),
    /// Pseudo-error to indicate program termination
    Terminate,
}

/// Result type for `RahmenError`
pub type RahmenResult<T> = Result<T, RahmenError>;

impl fmt::Display for RahmenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RahmenError::CaseUnknown(err) => write!(f, "Unknown case: {}", err),
            RahmenError::ConfigError(err) => err.fmt(f),
            RahmenError::IoError(err) => err.fmt(f),
            RahmenError::ImageError(err) => err.fmt(f),
            RahmenError::ParseFloatError(err) => err.fmt(f),
            RahmenError::PythonError(err) => write!(f, "Python error: {}", err),
            RahmenError::RegexError(err) => err.fmt(f),
            RahmenError::Retry => write!(f, "Retry"),
            RahmenError::Rexiv2Error(err) => err.fmt(f),
            RahmenError::Terminate => write!(f, "Terminate"),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::CaseUnknown(_err) => None,
            RahmenError::ConfigError(err) => err.source(),
            RahmenError::IoError(err) => err.source(),
            RahmenError::ImageError(err) => err.source(),
            RahmenError::ParseFloatError(err) => err.source(),
            RahmenError::PythonError(err) => err.source(),
            RahmenError::RegexError(err) => err.source(),
            RahmenError::Retry => None,
            RahmenError::Rexiv2Error(err) => err.source(),
            RahmenError::Terminate => None,
        }
    }
}

impl From<config::ConfigError> for RahmenError {
    fn from(err: config::ConfigError) -> Self {
        RahmenError::ConfigError(Arc::new(err))
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

impl From<ParseFloatError> for RahmenError {
    fn from(err: ParseFloatError) -> Self {
        RahmenError::ParseFloatError(err)
    }
}

impl From<pyo3::prelude::PyErr> for RahmenError {
    fn from(err: pyo3::prelude::PyErr) -> Self {
        RahmenError::PythonError(err)
    }
}

impl From<regex::Error> for RahmenError {
    fn from(err: regex::Error) -> Self {
        RahmenError::RegexError(err)
    }
}

impl From<rexiv2::Rexiv2Error> for RahmenError {
    fn from(err: rexiv2::Rexiv2Error) -> Self {
        RahmenError::Rexiv2Error(err)
    }
}
