use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RahmenError {
    Provider(ProviderError),
}

#[derive(Debug)]
pub enum ProviderError {
    Eof,
    Idle,
}

impl fmt::Display for RahmenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RahmenError::Provider(err) => err.fmt(f),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::Provider(err) => err.source(),
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            ProviderError::Idle => write!(f, "Provider idle"),
            ProviderError::Eof => write!(f, "Provider exhausted (Eof)"),
        }
    }
}

impl Error for ProviderError {}
pub type RahmenResult<T> = Result<T, RahmenError>;
