use super::*;
use framebuffer::{FixScreeninfo, VarScreeninfo};
use memmap::MmapMut;
use std::fs::File;

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
        frame: MmapMut::map_anon(len.max(16)).unwrap(),
        var_screen_info: VarScreeninfo {
            xres: 2,
            yres: 2,
            xres_virtual: 2,
            yres_virtual: 2,
            xoffset: 0,
            yoffset: 0,
            bits_per_pixel: 32,
            ..Default::default()
        },
        fix_screen_info: FixScreeninfo {
            line_length: 8,
            ..Default::default()
        },
    };
    let mut display = FramebufferDisplay::new(framebuffer);
    let source = RgbaImage::from_raw(2, 2, (1..=16).collect()).unwrap();
    display
        .render(0, Vector::new(0, 0), &DynamicImage::ImageRgba8(source))
        .unwrap();
    display.framebuffer.frame = MmapMut::map_anon(len).unwrap();
    display.framebuffer.var_screen_info.xres_virtual = virtual_width;
    display.framebuffer.var_screen_info.yres_virtual = virtual_height;
    display.framebuffer.var_screen_info.xoffset = x;
    display.framebuffer.var_screen_info.yoffset = y;
    display.framebuffer.fix_screen_info.line_length = stride;
    display.framebuffer.frame.fill(0xa5);
    display
}

#[test]
fn update_preserves_bgra_channel_order_in_tight_layout() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    display.update().unwrap();
    assert_eq!(
        &display.framebuffer.frame[..],
        &[3, 2, 1, 4, 7, 6, 5, 8, 11, 10, 9, 12, 15, 14, 13, 16]
    );
}

#[test]
fn update_leaves_virtual_rows_untouched() {
    let mut display = display(8, 2, 4, 0, 0, 32);
    display.update().unwrap();
    assert_eq!(&display.framebuffer.frame[..16], display.image.as_raw());
    assert_eq!(&display.framebuffer.frame[16..], &[0xa5; 16]);
}

#[test]
fn update_honors_padding_and_virtual_offsets() {
    let mut display = display(20, 4, 4, 1, 1, 80);
    let mut expected = vec![0xa5; 80];
    expected[24..32].copy_from_slice(&display.image.as_raw()[..8]);
    expected[44..52].copy_from_slice(&display.image.as_raw()[8..]);
    display.update().unwrap();
    assert_eq!(&display.framebuffer.frame[..], expected);
}

#[test]
fn update_rejects_invalid_layout_without_writes() {
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
            display.update().is_err(),
            "accepted {stride}/{width}/{height}/{x}/{y}/{len}"
        );
        assert!(display.framebuffer.frame.iter().all(|byte| *byte == 0xa5));
    }
}

#[test]
fn update_rejects_mismatched_backing_image_without_writes() {
    let mut display = display(8, 2, 2, 0, 0, 16);
    display.image = RgbaImage::new(1, 1);
    assert!(display.update().is_err());
    assert!(display.framebuffer.frame.iter().all(|byte| *byte == 0xa5));
}

#[test]
fn render_rejects_overflowing_geometry_before_allocating() {
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
