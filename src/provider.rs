//! Utilities to provide images, and other abstractions

use std::convert::{TryFrom, TryInto};
use std::io::BufReader;
use std::path::Path;

use convert_case::{Case, Casing};
use image::{DynamicImage, Pixel};
use itertools::Itertools;
use pyo3::prelude::*;
use pyo3::types::PyList;
use regex::Regex;
use rexiv2::Metadata;

use crate::config::{Element, Replacement};
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

/// settings for the status line formatter
#[derive(Debug, Deserialize, Clone)]
pub struct LineSettings {
    /// the separator to insert between the metadata
    pub separator: String,
    /// should we deduplicate metadata?
    pub uniquify: bool,
}

/// The following are the ops concerning the status line (text being displayed below the image)

/// Tries to convert a string slice to a Case
pub fn str_to_case(s: String) -> RahmenResult<Case> {
    let case_str = s.to_case(Case::Flat);
    for case in Case::all_cases() {
        if case_str == format!("{:?}", case).to_case(Case::Flat) {
            return Ok(case);
        }
    }
    Err(RahmenError::CaseUnknown(s))
}

/// abstract runtime definitions for the transformation ops for the meta data entries
#[derive(Debug)]
enum StatusLineTransformation {
    RegexReplace(Regex, String),
    Capitalize,
    ChangeCase(Case, Case),
}

/// runtime transformation ops for the metadata values (the parameters are gathered in the try_from function)
impl StatusLineTransformation {
    fn transform<S: AsRef<str>>(&self, input: S) -> String {
        match self {
            Self::RegexReplace(re, replacement) => re
                .replace_all(input.as_ref(), replacement.as_str())
                .into_owned(),
            Self::Capitalize => input.as_ref().from_case(Case::Upper).to_case(Case::Title),
            Self::ChangeCase(f, t) => input.as_ref().from_case(*f).to_case(*t),
        }
    }
}

/// prepare ops (regexes/replacements) to process the complete status line
impl TryFrom<Replacement> for StatusLineTransformation {
    type Error = RahmenError;
    /// build each status line element with its transformations and the tag
    fn try_from(value: Replacement) -> Result<Self, Self::Error> {
        // collect the transformation ops and store their parameters
        // iterate over the regex(es)

        Ok(StatusLineTransformation::RegexReplace(
            Regex::new(value.regex.as_ref())?,
            value.replace,
        ))
    }
}

/// a status line meta data element: a string and transformations to perform on it
#[derive(Debug)]
struct StatusLineElement {
    tags: Vec<String>,
    transformations: Vec<StatusLineTransformation>,
}

/// prepare the ops for the processing of an element
impl TryFrom<Element> for StatusLineElement {
    type Error = RahmenError;
    /// build each status line element with its transformations and the tag
    fn try_from(value: Element) -> Result<Self, Self::Error> {
        let mut transformations = vec![];
        // collect the transformation ops and store their parameters
        // the case conversion to apply
        if let Some(case_conversion) = value.case_conversion {
            transformations.push(StatusLineTransformation::ChangeCase(
                str_to_case(case_conversion.from)?,
                str_to_case(case_conversion.to)?,
            ));
        }
        // the capitalize instruction
        if value.capitalize.unwrap_or(false) {
            transformations.push(StatusLineTransformation::Capitalize);
        }
        // iterate over the regex(es)
        for replace in value.replace.into_iter().flat_map(Vec::into_iter) {
            transformations.push(StatusLineTransformation::RegexReplace(
                Regex::new(replace.regex.as_ref())?,
                replace.replace,
            ));
        }

        // return the transformations and the tags vector
        Ok(Self {
            transformations,
            tags: value.exif_tags,
        })
    }
}

/// the status line meta data element
impl StatusLineElement {
    /// this processes each metadata tag and subordinate instructions from the config file
    fn process(&self, metadata: &Metadata) -> Option<String> {
        // metadata processor: get the metadata value of the given meta tag (self.tag, from try_from above)
        // so we have three values here, self.tag (the tag), metadata (the data for this tag),
        // and value (the processed and later transformed metadata)
        // If the current metadata tag (self.tag.iter) can be converted to some value...
        if let Some(mut value) = self
            .tags
            .iter()
            // ...get tag as string...
            .map(|f| metadata.get_tag_interpreted_string(f).ok())
            // ...if it is s/th,...
            .find(Option::is_some)
            .flatten()
        // ...process that value using the pushed transformation ops and return the transformed value
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

/// A status line formatter formats meta data tags according to configured elements into a string
/// and then processes that string using regexes/replacements as configured
#[derive(Debug)]
pub struct StatusLineFormatter {
    // these are the meta tag entries in the config file
    elements: Vec<StatusLineElement>,
    // the separator string for the line output
    separator: String,
    // the Python code used to postprocess the metadata items
    py_postprocess_fn: Option<Py<PyAny>>,
}

impl StatusLineFormatter {
    /// Construct a new `StatusLineFormatter` from a collection of elements
    pub fn new<I: Iterator<Item = Element>>(
        // we get the arguments when we're called
        statusline_elements_iter: I,
        py_postprocess: Option<String>,
        separator: String,
    ) -> RahmenResult<Self> {
        // read the metadata config entries and store them to the elements vector
        let mut elements = vec![];
        for element in statusline_elements_iter {
            elements.push(element.try_into()?);
        }
        // read and store the Python code (if it exists)
        let py_postprocess_fn = if let Some(postprocess_path) = py_postprocess {
            Some(Python::with_gil(|py| {
                let module = py.import(postprocess_path.as_ref())?;
                module.call0("export").map(|obj| obj.into_py(py))
            })?)
        } else {
            None
        };

        Ok(Self {
            elements,
            py_postprocess_fn,
            separator,
        })
    }

    /// Format the meta data from the given path (called as receiver to the status line formatter)
    pub fn format<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> RahmenResult<String> {
        let metadata = Metadata::new_from_path(path)?;
        let mut line_elements = self
            .elements
            .iter()
            // process each metadata section (element) using the associated transformation instructions
            // empty tags (no metadata found): we will return an empty string to make sure all meta tags are
            // added to the status line. This way, we can postprocess the status line
            // being sure that parameters stay at their position.
            // This produces a Vec<String> of all the metadata found (empty strings if no data).
            .flat_map(move |element| {
                if let Some(v) = element.process(&metadata) {
                    Some(v)
                } else {
                    Some("".to_string())
                }
            })
            .collect();

        // postprocess the status line using a python function defined in the config file (if it exists)
        // this takes a Vec<String> of all the metadata found (empty strings if no data)
        // and produces a Vec<String> of either the items returned from the Python code (if there's some code),
        // or just the input
        line_elements = if let Some(code) = &self.py_postprocess_fn {
            Python::with_gil(|py| -> PyResult<Vec<String>> {
                let tags = PyList::new(py, &line_elements);
                code.call1(py, (tags, &self.separator))?.extract(py)
            })
            .unwrap()
        } else {
            // do nothing when there's no Python code
            line_elements
        };

        // unconditionally filter out the empty items we received from above and
        // deduplicate them, and join them with the separator, producing the final status line
        Ok(line_elements
            .iter()
            .filter(|x| !x.is_empty())
            .unique()
            .join(&self.separator))
    }
}
