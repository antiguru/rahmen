use super::*;
use framebuffer::{Bitfield, VarScreeninfo};
use image::{Rgb, Rgba};

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

fn bgra() -> PixelFormat {
    format(2, 1, 0, Some(3))
}

#[test]
fn jpeg_color_space_maps_byte_orders() {
    use mozjpeg::ColorSpace::*;
    assert_eq!(bgra().jpeg_color_space(), Some(JCS_EXT_BGRA));
    assert_eq!(format(0, 1, 2, None).jpeg_color_space(), Some(JCS_EXT_RGBA));
    assert_eq!(
        format(1, 2, 3, Some(0)).jpeg_color_space(),
        Some(JCS_EXT_ARGB)
    );
    assert_eq!(format(3, 2, 1, None).jpeg_color_space(), Some(JCS_EXT_ABGR));
    assert_eq!(format(1, 0, 2, Some(3)).jpeg_color_space(), None);
}

#[test]
fn black_sets_non_color_byte_opaque() {
    assert_eq!(bgra().black(), [0, 0, 0, 255]);
    assert_eq!(format(0, 1, 2, None).black(), [0, 0, 0, 255]);
    assert_eq!(format(1, 2, 3, None).black(), [255, 0, 0, 0]);
}

#[test]
fn encode_decode_roundtrip() {
    assert_eq!(bgra().encode([1, 2, 3]), [3, 2, 1, 255]);
    assert_eq!(bgra().decode([3, 2, 1, 255]), [1, 2, 3]);
    let argb = format(1, 2, 3, Some(0));
    assert_eq!(argb.decode(argb.encode([1, 2, 3])), [1, 2, 3]);
}

#[test]
fn resize_to_fit_passes_through_fitting_pictures() {
    let wide = Picture::from(DynamicImage::new_rgb8(4, 2));
    assert!(wide.resize_to_fit(4, 4).is_none());
    let tall = Picture::Native(bgra(), RgbaImage::new(2, 4));
    assert!(tall.resize_to_fit(4, 4).is_none());
}

#[test]
fn resize_to_fit_scales_images() {
    let small = Picture::from(DynamicImage::new_rgb8(2, 1));
    let resized = small.resize_to_fit(4, 4).unwrap();
    assert_eq!(resized.dimensions(), (4, 2));
    assert!(matches!(resized, Picture::Image(_)));
}

#[test]
fn resize_to_fit_scales_and_keeps_native_format() {
    let native = Picture::Native(bgra(), RgbaImage::from_pixel(8, 4, Rgba([3, 2, 1, 255])));
    let Some(Picture::Native(format, image)) = native.resize_to_fit(4, 4) else {
        panic!("expected a native picture");
    };
    assert_eq!(format, bgra());
    assert_eq!(image.dimensions(), (4, 2));
    assert!(image.pixels().all(|pixel| pixel.0 == [3, 2, 1, 255]));
}

#[test]
fn native_to_rgb8_uses_layout() {
    let native = Picture::Native(bgra(), RgbaImage::from_pixel(1, 1, Rgba([3, 2, 1, 255])));
    assert_eq!(native.to_rgb8().get_pixel(0, 0), &Rgb([1, 2, 3]));
}
