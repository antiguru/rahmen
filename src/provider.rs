//! Utilities to provide images, and other abstractions

use std::convert::{TryFrom, TryInto};
use std::io::BufReader;
use std::path::Path;

use convert_case::{Case, Casing};
use image::{DynamicImage, Pixel};
use itertools::Itertools;
use regex::Regex;
use rexiv2::Metadata;

use crate::config::Element;
use crate::errors::{RahmenError, RahmenResult};

/// Provider trait to produce images, or other types
pub trait Provider<D> {
    /// Obtain the next element.
    /// Error -> Terminate
    /// Ok(Some(T)) -> Process T
    /// Ok(None) -> Exhausted
    fn next_image(&mut self) -> RahmenResult<Option<D>>;
}

impl<D> Provider<D> for Box<dyn Provider<D>> {
    fn next_image(&mut self) -> RahmenResult<Option<D>> {
        (**self).next_image()
    }
}

fn load_jpeg<P: AsRef<Path>>(path: P, max_size: Option<usize>) -> RahmenResult<DynamicImage> {
    let mut d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_path(&path)?;

    if let Some(max_size) = max_size {
        let mut scale = 8;
        let ratio_to_max_size = max_size as f32 / (d.width() * d.height()) as f32;
        if ratio_to_max_size < 1. {
            scale = (ratio_to_max_size * 8.) as u8 + 1;
        }
        d.scale(scale);
    }
    let mut decompress_started = d.to_colorspace(mozjpeg::ColorSpace::JCS_EXT_BGR)?;
    let height = decompress_started.height();
    let mut img = DynamicImage::new_bgr8(decompress_started.width() as _, height as _);
    let buffer: Option<Vec<[u8; 3]>> = decompress_started.read_scanlines();
    let rgb_img = img.as_mut_bgr8().unwrap();
    if let Some(buffer) = buffer {
        for (row, row_buffer) in buffer.chunks(buffer.len() / height).enumerate() {
            for (col, pixel) in row_buffer.iter().enumerate() {
                *rgb_img.get_pixel_mut(col as _, row as _) = *image::Bgr::from_slice(pixel);
            }
        }
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Retry)
    }
}

/// Load an image from a path
pub fn load_image_from_path<P: AsRef<Path>>(
    path: P,
    max_size: Option<usize>,
) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| println!("Loading {}ms", e.as_millis()));
    println!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path, max_size),
        format => {
            image::io::Reader::with_format(BufReader::new(std::fs::File::open(&path)?), format)
                .decode()
                .map_err(Into::into)
        }
    }
}
/// Tries to convert a string slice to a Case
/// TODO convert to RahmenError and remove the unwraps where this is called
pub fn str_to_case(s: &str) -> Result<Case, String> {
    let case_str = s.to_case(Case::Flat);
    for case in Case::all_cases() {
        if case_str == format!("{:?}", case).to_case(Case::Flat) {
            return Ok(case);
        }
    }
    Err(format!("Unknown Case for conversion: {:}", &s))
}

#[derive(Debug)]
enum StatusLineTransformation {
    RegexReplace(Regex, String),
    Capitalize,
    ChangeCase(String, String),
}

impl StatusLineTransformation {
    fn transform<S: AsRef<str>>(&self, input: S) -> String {
        match self {
            Self::RegexReplace(re, replacement) => re
                .replace_all(input.as_ref(), replacement.as_str())
                .into_owned(),
            Self::Capitalize => input.as_ref().from_case(Case::Upper).to_case(Case::Title),
            Self::ChangeCase(fr, t) => input
                .as_ref()
                .from_case(str_to_case(fr).unwrap())
                .to_case(str_to_case(t).unwrap()),
        }
    }
}

#[derive(Debug)]
struct StatusLineElement {
    tags: Vec<String>,
    transformations: Vec<StatusLineTransformation>,
}

impl TryFrom<Element> for StatusLineElement {
    type Error = RahmenError;

    fn try_from(value: Element) -> Result<Self, Self::Error> {
        let mut transformations = vec![];
        if value.case_from.is_some() || value.case_to.is_some() {
            transformations.push(StatusLineTransformation::ChangeCase(
                // what goes here?
                value.case_from.expect("From case missing"),
                value.case_to.expect("To case missing"),
            ));
        }
        if value.capitalize.unwrap_or(false) {
            transformations.push(StatusLineTransformation::Capitalize);
        }
        for replace in value.replace.into_iter().flat_map(Vec::into_iter) {
            transformations.push(StatusLineTransformation::RegexReplace(
                Regex::new(replace.regex.as_ref())?,
                replace.replace,
            ));
        }
        Ok(Self {
            transformations,
            tags: value.exif_tags,
        })
    }
}

impl StatusLineElement {
    fn process(&self, metadata: &Metadata) -> Option<String> {
        if let Some(mut value) = self
            .tags
            .iter()
            .filter(|f| metadata.has_tag(f))
            .map(|f| metadata.get_tag_interpreted_string(f).ok())
            .find(Option::is_some)
            .flatten()
        {
            for transformation in &self.transformations {
                value = transformation.transform(value);
            }
            Some(value)
        } else {
            None
        }
    }
}

/// A status line formatter formats meta data tags according to configured elements
#[derive(Debug)]
pub struct StatusLineFormatter {
    elements: Vec<StatusLineElement>,
}

impl StatusLineFormatter {
    /// Construct a new `StatusLineFormatter` from a collection of elements
    pub fn new<I: Iterator<Item = Element>>(elements_iter: I) -> RahmenResult<Self> {
        let mut elements = vec![];
        for element in elements_iter {
            elements.push(element.try_into()?);
        }
        Ok(Self { elements })
    }

    /// Format the meta data from the given path using this formatter
    pub fn format<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> RahmenResult<String> {
        let metadata = Metadata::new_from_path(path)?;
        // iterate over the tag table
        Ok(self
            .elements
            .iter()
            .flat_map(move |element| element.process(&metadata))
            // remove empty strings (which may be the result of a transformation regex replacement)
            .filter(|x| !x.is_empty())
            // remove multiples (e.g. if City and  ProvinceState are the same)
            .unique()
            .join(", "))
    }
}
