//! Minimal safe wrapper over libexiv2.
//!
//! A thin C++ shim (see `shim.{h,cc}`) bound through the `cxx` crate. Only the
//! read-only operations rahmen actually uses are exposed: loading an image and
//! reading a single tag as an interpreted string. The unavoidable FFI `unsafe`
//! is confined to this crate so the rahmen crate can keep `forbid(unsafe_code)`.

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;

use cxx::UniquePtr;

// SAFETY: this is the cxx FFI bridge. The `unsafe extern "C++"` block only
// declares the signatures of the C++ functions in `shim.cc`; cxx generates the
// marshalling glue and verifies the C++ side matches at compile time. The
// declared functions are memory-safe: they take borrowed strings/handles and
// return owned values, and all C++ exceptions are caught and surfaced as Err.
#[cxx::bridge(namespace = "rahmen_exiv2")]
mod ffi {
    unsafe extern "C++" {
        include!("src/shim.h");

        /// Loaded exiv2 image with its metadata read.
        type Image;

        /// Open `path` and read its metadata.
        fn open_image(path: &str) -> Result<UniquePtr<Image>>;

        /// Interpreted (human-readable) string for `key`.
        fn tag_interpreted(image: &Image, key: &str) -> Result<String>;
    }
}

/// Error originating from the exiv2 wrapper.
#[derive(Debug)]
pub struct Exiv2Error(String);

impl fmt::Display for Exiv2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exiv2 error: {}", self.0)
    }
}

impl Error for Exiv2Error {}

/// Image metadata loaded from a file.
pub struct Metadata {
    image: UniquePtr<ffi::Image>,
}

impl Metadata {
    /// Load metadata from the file at `path`.
    pub fn new_from_path<P: AsRef<OsStr>>(path: P) -> Result<Self, Exiv2Error> {
        // exiv2 takes a UTF-8 path; reject non-UTF-8 rather than lose bytes.
        let path = path
            .as_ref()
            .to_str()
            .ok_or_else(|| Exiv2Error("path is not valid UTF-8".to_string()))?;
        let image = ffi::open_image(path).map_err(|e| Exiv2Error(e.what().to_string()))?;
        Ok(Self { image })
    }

    /// Read `tag` as an interpreted string. Returns an error when the tag is
    /// absent or the key is invalid.
    pub fn get_tag_interpreted_string(&self, tag: &str) -> Result<String, Exiv2Error> {
        ffi::tag_interpreted(&self.image, tag).map_err(|e| Exiv2Error(e.what().to_string()))
    }
}
