use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RahmenError {
    Retry,
    Terminate,
}

impl fmt::Display for RahmenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RahmenError::Retry => write!(f, "Retry requested"),
            RahmenError::Terminate => write!(f, "Termination requested"),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::Retry => None,
            RahmenError::Terminate => None,
        }
    }
}

pub type RahmenResult<T> = Result<T, RahmenError>;
