//! Utilities to provide images, and other abstractions

use std::io::BufReader;
use std::path::Path;

use convert_case::{Case, Casing};
use image::{DynamicImage, Pixel};
use itertools::Itertools;
use regex::Regex;
use rexiv2::Metadata;

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

const FIELD_LOOKUP_TABLE: &[&[&str]] = &[
    &["Iptc.Application2.ObjectName"],
    &["Iptc.Application2.SubLocation"],
    &["Iptc.Application2.City"],
    &["Iptc.Application2.ProvinceState"],
    &["Iptc.Application2.CountryName"],
    &["Exif.Photo.DateTimeOriginal"],
    &["Xmp.dc.creator"],
];

/// process metadata tags to beautify the status line
/* TODO this is too generic, it's ugly that it processes data that we know it doesn't have to, and it's not possible to narrow
   it down to specific metadata tags this way (e.g., return only first word of creator tag) -> how do we get access to the tag name?
   Insert the map where we're called further up in the iterator over the tag values?
   also, make this configurable
   also, harness a function to reuse the repeated stuff
   also, what about redefining re and s?
*/
pub fn process_tag(tag: &String) -> String {
    // convert date to German format
    let re = Regex::new(r"(?P<y>\d{4})[-:](?P<m>\d{2})[-:](?P<d>\d{2})").unwrap();
    // TODO find better way to insert comma after year, might lead to a surplus comma if no time is found in metadata? (but Exif.Photo.DateTimeOriginal _should_ contain a time)
    let s = re.replace_all(tag, "$d.$m.$y,").into_owned();
    // remove seconds from time
    let re = Regex::new(r"(?P<h>\d{2}):(?P<m>\d{2}):(?P<s>\d{2})").unwrap();
    let s = re.replace_all(&s, "$h:$m").into_owned();
    // remove leading zeros after whitespace/dot (date)
    let re = Regex::new(r"[\s.]0").unwrap();
    let s = re.replace_all(&s, ".").into_owned();
    // remove www stuff
    let re = Regex::new(r"\b<?www.").unwrap();
    let s = re.replace_all(&s, "").into_owned();
    // remove numeric strings starting with plus sign after whitespace (phone numbers)
    let re = Regex::new(r"\s\+\d+").unwrap();
    // and convert from UPPER CASE to Title Case
    re.replace_all(&s, "")
        .into_owned()
        .from_case(Case::Upper)
        .to_case(Case::Title)
}

/// Format the metadata tags from an image to show a status line
pub fn format_exif<P: AsRef<std::ffi::OsStr>>(path: P) -> RahmenResult<String> {
    let metadata = Metadata::new_from_path(path)?;
    // iterate over the tag table
    let tag_values = FIELD_LOOKUP_TABLE
        .iter()
        .flat_map(move |lookup| {
            lookup
                //iterate over each exif result, check if the tag is available  and return value if one exists
                .iter()
                .filter(|f| metadata.has_tag(f))
                .map(|f| metadata.get_tag_interpreted_string(*f).ok())
                .find(Option::is_some)
                .map(Option::unwrap)
        })
        // remove multiples (e.g. if City and  ProvinceState are the same)
        .unique()
        .map(|tag| process_tag(&tag))
        .collect::<Vec<String>>();
    println!("{:?}", tag_values);
    Ok(tag_values.join(", "))
}
