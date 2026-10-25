use super::load_image_from_path;
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(width: u32, height: u32, format: ImageFormat) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rahmen-budget-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            format.extensions_str()[0]
        ));
        DynamicImage::ImageRgb8(RgbImage::new(width, height))
            .save_with_format(&path, format)
            .unwrap();
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
