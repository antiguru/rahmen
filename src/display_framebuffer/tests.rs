use super::*;
use framebuffer::{FixScreeninfo, VarScreeninfo};
use image::{RgbImage, RgbaImage};
use memmap2::MmapMut;
use std::fs::File;

fn bitfield(offset: u32, length: u32) -> Bitfield {
    Bitfield {
        offset,
        length,
        msb_right: 0,
    }
}

/// A 2x2 display in the BGRA layout `fbset` reports as `rgba 8/16,8/8,8/0,8/24`.
fn display(
    stride: u32,
    virtual_width: u32,
    virtual_height: u32,
    x: u32,
    y: u32,
    len: usize,
) -> FramebufferDisplay {
    let framebuffer = Framebuffer {
        device: File::open("/dev/zero").unwrap(),
        frame: MmapMut::map_anon(len.max(1)).unwrap(),
        var_screen_info: VarScreeninfo {
            xres: 2,
            yres: 2,
            xres_virtual: virtual_width,
            yres_virtual: virtual_height,
            xoffset: x,
            yoffset: y,
            bits_per_pixel: 32,
            red: bitfield(16, 8),
            green: bitfield(8, 8),
            blue: bitfield(0, 8),
            transp: bitfield(24, 8),
            ..Default::default()
        },
        fix_screen_info: FixScreeninfo {
            line_length: stride,
            ..Default::default()
        },
    };
    let mut display = FramebufferDisplay::new(framebuffer);
    display.framebuffer.frame.fill(0xa5);
    display
}

/// A 2x2 RGB image with pixels (1, 2, 3), (4, 5, 6), ...
fn rgb_image() -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_raw(2, 2, (1..=12).collect()).unwrap())
}

const BGRA: [u8; 16] = [3, 2, 1, 255, 6, 5, 4, 255, 9, 8, 7, 255, 12, 11, 10, 255];

#[test]
fn render_converts_rgb_to_framebuffer_layout() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    display.render(0, Vector::new(0, 0), &rgb_image()).unwrap();
    assert_eq!(&display.framebuffer.frame[..], &BGRA);
}

#[test]
fn render_drops_alpha_of_rgba_images() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    let source = RgbaImage::from_raw(2, 2, (1..=16).collect()).unwrap();
    display
        .render(0, Vector::new(0, 0), &DynamicImage::ImageRgba8(source))
        .unwrap();
    assert_eq!(
        &display.framebuffer.frame[..],
        &[3, 2, 1, 255, 7, 6, 5, 255, 11, 10, 9, 255, 15, 14, 13, 255]
    );
}

#[test]
fn render_follows_reported_channel_offsets() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    let var = &mut display.framebuffer.var_screen_info;
    var.red = bitfield(0, 8);
    var.blue = bitfield(16, 8);
    var.transp = bitfield(0, 0);
    display.render(0, Vector::new(0, 0), &rgb_image()).unwrap();
    assert_eq!(
        &display.framebuffer.frame[..],
        &[1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 0, 10, 11, 12, 0]
    );
}

#[test]
fn render_leaves_virtual_rows_untouched() {
    let mut display = display(8, 2, 4, 0, 0, 32);
    display.render(0, Vector::new(0, 0), &rgb_image()).unwrap();
    assert_eq!(&display.framebuffer.frame[..16], &BGRA);
    assert_eq!(&display.framebuffer.frame[16..], &[0xa5; 16]);
}

#[test]
fn render_honors_padding_and_virtual_offsets() {
    let mut display = display(20, 4, 4, 1, 1, 80);
    let mut expected = vec![0xa5; 80];
    expected[24..32].copy_from_slice(&BGRA[..8]);
    expected[44..52].copy_from_slice(&BGRA[8..]);
    display.render(0, Vector::new(0, 0), &rgb_image()).unwrap();
    assert_eq!(&display.framebuffer.frame[..], expected);
}

#[test]
fn render_clips_to_the_screen() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    display.render(0, Vector::new(1, -1), &rgb_image()).unwrap();
    let mut expected = [0xa5; 16];
    expected[4..8].copy_from_slice(&BGRA[8..12]);
    assert_eq!(&display.framebuffer.frame[..], &expected);

    display.render(0, Vector::new(2, 0), &rgb_image()).unwrap();
    display.render(0, Vector::new(-2, 0), &rgb_image()).unwrap();
    assert_eq!(&display.framebuffer.frame[..], &expected);
}

#[test]
fn blank_writes_opaque_black() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    display
        .blank(0, Vector::new(1, 0), Vector::new(5, 1))
        .unwrap();
    let mut expected = [0xa5; 16];
    expected[4..8].copy_from_slice(&[0, 0, 0, 255]);
    assert_eq!(&display.framebuffer.frame[..], &expected);
}

#[test]
fn render_rejects_invalid_layout_without_writes() {
    for (stride, width, height, x, y, len) in [
        (4, 2, 2, 0, 0, 16),
        (8, 2, 2, 0, 0, 15),
        (16, 3, 2, 2, 0, 32),
        (8, 2, 3, 0, 2, 24),
        (8, 2, 2, u32::MAX, 0, 16),
        (8, 2, 2, 0, u32::MAX, 16),
        (u32::MAX, 2, u32::MAX, 0, u32::MAX - 2, 16),
    ] {
        let mut display = display(stride, width, height, x, y, len);
        assert!(
            display.render(0, Vector::new(0, 0), &rgb_image()).is_err(),
            "accepted {stride}/{width}/{height}/{x}/{y}/{len}"
        );
        assert!(display.framebuffer.frame.iter().all(|byte| *byte == 0xa5));
    }
}

#[test]
fn render_rejects_unsupported_pixel_formats_without_writes() {
    for (red, green, blue, transp) in [
        // RGB565-like channel widths
        (
            bitfield(11, 5),
            bitfield(5, 6),
            bitfield(0, 5),
            bitfield(0, 0),
        ),
        // Unaligned channel
        (
            bitfield(20, 8),
            bitfield(8, 8),
            bitfield(0, 8),
            bitfield(0, 0),
        ),
        // Two channels sharing a byte
        (
            bitfield(16, 8),
            bitfield(16, 8),
            bitfield(0, 8),
            bitfield(24, 8),
        ),
        (
            bitfield(16, 8),
            bitfield(8, 8),
            bitfield(0, 8),
            bitfield(0, 8),
        ),
        // Missing color channel
        (
            bitfield(0, 0),
            bitfield(8, 8),
            bitfield(0, 8),
            bitfield(24, 8),
        ),
    ] {
        let mut display = display(8, 2, 2, 0, 0, 16);
        let var = &mut display.framebuffer.var_screen_info;
        (var.red, var.green, var.blue, var.transp) = (red, green, blue, transp);
        assert!(display.render(0, Vector::new(0, 0), &rgb_image()).is_err());
        assert!(display.framebuffer.frame.iter().all(|byte| *byte == 0xa5));
    }
}

#[test]
fn render_rejects_overflowing_geometry_without_writes() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    let var = &mut display.framebuffer.var_screen_info;
    var.xres = u32::MAX;
    var.yres = u32::MAX;
    var.xres_virtual = u32::MAX;
    var.yres_virtual = u32::MAX;
    display.framebuffer.fix_screen_info.line_length = u32::MAX;
    assert!(
        display
            .render(0, Vector::new(0, 0), &DynamicImage::new_rgba8(1, 1))
            .is_err()
    );
    assert!(display.framebuffer.frame.iter().all(|byte| *byte == 0xa5));
}
