use std::io::BufReader;
use std::path::Path;

use exif::{Field, Value};
use image::{DynamicImage, Pixel};

use crate::errors::{RahmenError, RahmenResult};

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

fn load_jpeg<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_path(&path)?;
    let mut img = DynamicImage::new_bgra8(d.width() as _, d.height() as _);
    let height = d.height();
    let buffer: Option<Vec<[u8; 4]>> = d
        .to_colorspace(mozjpeg::ColorSpace::JCS_EXT_BGRA)?
        .read_scanlines();
    let rgba_img = img.as_mut_bgra8().unwrap();
    if let Some(buffer) = buffer {
        for (row, row_buffer) in buffer.chunks(buffer.len() / height).enumerate() {
            for (col, pixel) in row_buffer.iter().enumerate() {
                *rgba_img.get_pixel_mut(col as _, row as _) = *image::Bgra::from_slice(pixel);
            }
        }
        Ok(img)
    } else {
        eprintln!("Failed to decode image: {:?}", path.as_ref());
        Err(RahmenError::Retry.into())
    }
}

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<DynamicImage> {
    let _t = crate::Timer::new(|e| println!("Loading {}ms", e.as_millis()));
    println!("Loading {:?}", path.as_ref());
    match image::ImageFormat::from_path(&path)? {
        image::ImageFormat::Jpeg => load_jpeg(path),
        format => {
            image::io::Reader::with_format(BufReader::new(std::fs::File::open(&path)?), format)
                .decode()
                .map_err(Into::into)
        }
    }
}

pub type Coordinate = (f64, f64);

pub fn coordinates_from_exif<'a, I: Iterator<Item = &'a Field>>(mut iter: I) -> Option<Coordinate> {
    fn parse_coordinates(value: &Value) -> Option<f64> {
        match value {
            Value::Rational(rationals) => {
                if rationals.len() != 3 {
                    None
                } else {
                    Some(
                        rationals[0].to_f64()
                            + rationals[1].to_f64() / 60f64
                            + rationals[2].to_f64() / (60f64 * 6f64),
                    )
                }
            }
            _ => None,
        }
    }
    fn parse_direction_modifier(value: &Value) -> Option<f64> {
        if let Value::Ascii(c) = value {
            match c.get(0).and_then(|c| c.get(0)) {
                Some(b'N') => Some(1f64),
                Some(b'S') => Some(-1f64),
                Some(b'E') => Some(1f64),
                Some(b'W') => Some(-1f64),
                _ => None,
            }
        } else {
            None
        }
    }
    let mut gps_latitude_ref = None;
    let mut gps_latitude = None;
    let mut gps_longitude_ref = None;
    let mut gps_longitude = None;
    while let Some(field) = iter.next() {
        match field.tag {
            exif::Tag::GPSLatitudeRef => gps_latitude_ref = parse_direction_modifier(&field.value),
            exif::Tag::GPSLatitude => gps_latitude = parse_coordinates(&field.value),
            exif::Tag::GPSLongitudeRef => {
                gps_longitude_ref = parse_direction_modifier(&field.value)
            }
            exif::Tag::GPSLongitude => gps_longitude = parse_coordinates(&field.value),
            _ => {}
        }
    }
    let latitude = gps_latitude.and_then(|l| gps_latitude_ref.and_then(|r| Some(r * l)));
    let longitude = gps_longitude.and_then(|l| gps_longitude_ref.and_then(|r| Some(r * l)));
    latitude.and_then(|latitude| longitude.and_then(|longitude| Some((latitude, longitude))))
}

mod location_lookup {
    lazy_static! {
        pub(crate) static ref LOCATIONS: reverse_geocoder::Locations =
            reverse_geocoder::Locations::from_memory();
        pub(crate) static ref GEOCODER: reverse_geocoder::ReverseGeocoder<'static> =
            reverse_geocoder::ReverseGeocoder::new(&LOCATIONS);
    }
}

pub fn coordinates_to_location(coordinate: Coordinate) -> Option<String> {
    location_lookup::GEOCODER
        .search(coordinate)
        .map(|result| result.record.name.clone())

    // if let Some(search_result) = search_result {
    //     println!(
    //         "Location: {:?} {:?}",
    //         search_result.distance, search_result.record
    //     );
    //     Some(search_result.record.name.clone())
    // } else {
    //     None
    // }
}

pub fn read_exif_from_path<P: AsRef<Path>>(path: P) -> RahmenResult<Vec<exif::Field>> {
    let file = std::fs::File::open(path)?;
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = exif::Reader::new();
    exifreader
        .read_from_container(&mut bufreader)
        .map(|exif| exif.fields().cloned().collect::<Vec<_>>())
        .map_err(Into::into)

    // if let Some(coordinate) = coordinates_from_exif(exif.fields()) {
    //     coordinates_to_location(coordinate);
    // }
    // for f in exif.fields() {
    //     println!(
    //         "{} {} {}",
    //         f.tag,
    //         f.ifd_num,
    //         f.display_value().with_unit(&exif)
    //     );
    // }
}
