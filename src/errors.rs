use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RahmenError {
    FramebufferError(framebuffer::FramebufferError),
    ImageError(image::error::ImageError),
    IoError(std::io::Error),
    LinuxFBError(linuxfb::Error),
    #[cfg(feature = "minifb")]
    MiniFBError(minifb::Error),
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
            RahmenError::FramebufferError(err) => err.fmt(f),
            RahmenError::ImageError(err) => err.fmt(f),
            RahmenError::IoError(err) => err.fmt(f),
            RahmenError::LinuxFBError(_err) => f.write_str("linuxfb::Error"),
            #[cfg(feature = "minifb")]
            RahmenError::MiniFBError(err) => err.fmt(f),
            RahmenError::Provider(err) => err.fmt(f),
        }
    }
}

impl Error for RahmenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RahmenError::FramebufferError(err) => err.source(),
            RahmenError::ImageError(err) => err.source(),
            RahmenError::IoError(err) => err.source(),
            RahmenError::LinuxFBError(_err) => None,
            #[cfg(feature = "minifb")]
            RahmenError::MiniFBError(err) => err.source(),
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

impl From<linuxfb::Error> for RahmenError {
    fn from(e: linuxfb::Error) -> Self {
        RahmenError::LinuxFBError(e)
    }
}

impl From<framebuffer::FramebufferError> for RahmenError {
    fn from(e: framebuffer::FramebufferError) -> Self {
        RahmenError::FramebufferError(e)
    }
}

#[cfg(feature = "minifb")]
impl From<minifb::Error> for RahmenError {
    fn from(e: minifb::Error) -> Self {
        RahmenError::MiniFBError(e)
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
