use super::{LoadTarget, load_image_from_path, load_picture};
use crate::picture::{Picture, PixelFormat};
use framebuffer::{Bitfield, VarScreeninfo};
use image::{DynamicImage, GenericImageView, GrayImage, ImageFormat, Luma, Rgb, RgbImage};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(width: u32, height: u32, format: ImageFormat) -> Self {
        Self::with_image(
            DynamicImage::ImageRgb8(RgbImage::new(width, height)),
            format,
        )
    }

    fn with_image(image: DynamicImage, format: ImageFormat) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rahmen-budget-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            format.extensions_str()[0]
        ));
        image.save_with_format(&path, format).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn jpeg_selects_largest_scale_using_rgb_bytes() {
    let file = Fixture::new(100, 100, ImageFormat::Jpeg);
    for (budget, dimensions) in [
        (30_000, (100, 100)),
        (16_000, (63, 63)),
        (7_500, (50, 50)),
        (507, (13, 13)),
    ] {
        let decoded = load_image_from_path(&file.0, Some(budget)).unwrap();
        assert_eq!(decoded.dimensions(), dimensions);
        assert!(decoded.as_bytes().len() <= budget);
    }
}

#[test]
fn jpeg_rounds_nonmultiple_dimensions_before_counting_bytes() {
    let file = Fixture::new(17, 19, ImageFormat::Jpeg);
    for (budget, dimensions) in [(27, (3, 3)), (75, (5, 5)), (969, (17, 19))] {
        let decoded = load_image_from_path(&file.0, Some(budget)).unwrap();
        assert_eq!(decoded.dimensions(), dimensions);
        assert!(decoded.as_bytes().len() <= budget);
    }
}

#[test]
fn jpeg_rejects_budgets_below_smallest_scale() {
    let file = Fixture::new(100, 100, ImageFormat::Jpeg);
    for budget in [0, 1, 2, 506] {
        assert!(
            load_image_from_path(&file.0, Some(budget)).is_err(),
            "budget {budget}"
        );
    }
}

#[test]
fn png_rejects_oversized_raster() {
    let file = Fixture::new(100, 100, ImageFormat::Png);
    for budget in [0, 1, 1_000, 29_999] {
        assert!(
            load_image_from_path(&file.0, Some(budget)).is_err(),
            "budget {budget}"
        );
    }
    let decoded = load_image_from_path(&file.0, Some(30_000)).unwrap();
    assert_eq!(decoded.as_bytes().len(), 30_000);
}

#[test]
fn unlimited_preserves_original_dimensions() {
    for format in [ImageFormat::Jpeg, ImageFormat::Png] {
        let file = Fixture::new(17, 19, format);
        let decoded = load_image_from_path(&file.0, None).unwrap();
        assert_eq!(decoded.dimensions(), (17, 19));
        assert_eq!(decoded.as_bytes().len(), 969);
    }
}

#[test]
fn dimension_byte_arithmetic_rejects_overflow() {
    assert!(super::check_budget(usize::MAX, 2, 3, None).is_err());
    assert!(super::check_budget(usize::MAX / 2, 1, 3, Some(usize::MAX)).is_err());
}

/// A pixel format with channels at the given byte offsets of a little-endian 32-bit pixel.
fn format(red: u32, green: u32, blue: u32, alpha: Option<u32>) -> PixelFormat {
    let field = |byte: u32| Bitfield {
        offset: byte * 8,
        length: 8,
        msb_right: 0,
    };
    PixelFormat::from_var_screen_info(&VarScreeninfo {
        bits_per_pixel: 32,
        red: field(red),
        green: field(green),
        blue: field(blue),
        transp: alpha.map(field).unwrap_or_default(),
        ..Default::default()
    })
    .unwrap()
}

fn target(format: PixelFormat, size: (u32, u32)) -> Option<LoadTarget> {
    Some(LoadTarget {
        format: Some(format),
        size,
    })
}

fn red_jpeg(width: u32, height: u32) -> Fixture {
    let image = RgbImage::from_pixel(width, height, Rgb([255, 0, 0]));
    Fixture::with_image(DynamicImage::ImageRgb8(image), ImageFormat::Jpeg)
}

#[test]
fn jpeg_decodes_into_native_layout() {
    let file = red_jpeg(100, 100);
    let bgra = format(2, 1, 0, Some(3));
    let picture = load_picture(&file.0, None, target(bgra, (100, 100))).unwrap();
    let Picture::Native(decoded_format, image) = picture else {
        panic!("expected a native picture");
    };
    assert_eq!(decoded_format, bgra);
    assert_eq!(image.dimensions(), (100, 100));
    assert!(image.pixels().all(|pixel| {
        let [blue, green, red, alpha] = pixel.0;
        blue <= 8 && green <= 8 && red >= 247 && alpha == 255
    }));
}

#[test]
fn grayscale_jpeg_decodes_into_native_layout() {
    let image = DynamicImage::ImageLuma8(GrayImage::from_pixel(8, 8, Luma([200])));
    let file = Fixture::with_image(image, ImageFormat::Jpeg);
    let picture = load_picture(&file.0, None, target(format(2, 1, 0, Some(3)), (8, 8))).unwrap();
    let Picture::Native(_, image) = picture else {
        panic!("expected a native picture");
    };
    assert!(image.pixels().all(|pixel| {
        let [blue, green, red, alpha] = pixel.0;
        blue == green && green == red && red.abs_diff(200) <= 2 && alpha == 255
    }));
}

#[test]
fn jpeg_native_budget_counts_four_bytes() {
    let file = red_jpeg(100, 100);
    let bgra = format(2, 1, 0, Some(3));
    let native = Some((bgra, bgra.jpeg_color_space().unwrap()));
    // At 4 bytes per pixel, 88x88 needs 30 976 bytes and 75x75 fits. Counting 3 bytes per
    // pixel would have selected 88x88.
    let picture = super::load_jpeg(&file.0, Some(30_000), native).unwrap();
    let Picture::Native(_, image) = picture else {
        panic!("expected a native picture");
    };
    assert_eq!(image.dimensions(), (75, 75));
    assert!(image.as_raw().len() <= 30_000);
}

#[test]
fn load_resizes_to_target() {
    let file = red_jpeg(100, 50);
    let picture = load_picture(&file.0, None, target(format(2, 1, 0, Some(3)), (40, 40))).unwrap();
    assert_eq!(picture.dimensions(), (40, 20));
}

#[test]
fn png_with_native_target_stays_image() {
    let file = Fixture::new(100, 50, ImageFormat::Png);
    let picture = load_picture(&file.0, None, target(format(2, 1, 0, Some(3)), (40, 40))).unwrap();
    assert!(matches!(picture, Picture::Image(_)));
    assert_eq!(picture.dimensions(), (40, 20));
}

#[test]
fn unsupported_layout_decodes_rgb() {
    let file = red_jpeg(10, 10);
    let picture = load_picture(&file.0, None, target(format(1, 0, 2, Some(3)), (10, 10))).unwrap();
    assert!(matches!(
        picture,
        Picture::Image(DynamicImage::ImageRgb8(_))
    ));
}

#[test]
fn load_without_target_keeps_size() {
    let file = red_jpeg(17, 19);
    let picture = load_picture(&file.0, None, None).unwrap();
    assert!(matches!(picture, Picture::Image(_)));
    assert_eq!(picture.dimensions(), (17, 19));
}
