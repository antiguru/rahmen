//! Functionality to render images on a Linux framebuffer

use crate::display::Display;
use crate::errors::{RahmenError, RahmenResult};

use crate::Vector;
use framebuffer::{Bitfield, Framebuffer, VarScreeninfo};
use image::{DynamicImage, ImageBuffer, Pixel};
use std::time::Duration;

/// A display driver for Linux framebuffers.
///
/// Images are converted to the framebuffer's pixel format, as reported by the device, and
/// written directly into the mapped framebuffer memory.
#[derive(Debug)]
pub struct FramebufferDisplay {
    framebuffer: Framebuffer,
}

/// Byte positions of the color channels within a 32-bit framebuffer pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelFormat {
    red: usize,
    green: usize,
    blue: usize,
    alpha: Option<usize>,
}

impl PixelFormat {
    fn new(var: &VarScreeninfo) -> RahmenResult<Self> {
        let red = channel_byte(&var.red)?.ok_or_else(invalid_layout)?;
        let green = channel_byte(&var.green)?.ok_or_else(invalid_layout)?;
        let blue = channel_byte(&var.blue)?.ok_or_else(invalid_layout)?;
        let alpha = channel_byte(&var.transp)?;
        let mut bytes = [Some(red), Some(green), Some(blue), alpha];
        bytes.sort();
        if bytes.windows(2).any(|w| w[0].is_some() && w[0] == w[1]) {
            return Err(invalid_layout());
        }
        Ok(Self {
            red,
            green,
            blue,
            alpha,
        })
    }

    /// An opaque black pixel, used as the base for every converted pixel.
    fn black(&self) -> [u8; 4] {
        let mut pixel = [0; 4];
        if let Some(alpha) = self.alpha {
            pixel[alpha] = u8::MAX;
        }
        pixel
    }
}

/// Map a channel's bit field to its byte position within a 32-bit pixel. Only byte-aligned
/// 8-bit channels are supported. Returns `None` for an absent channel.
fn channel_byte(field: &Bitfield) -> RahmenResult<Option<usize>> {
    if field.length == 0 {
        return Ok(None);
    }
    if field.length != 8 || field.offset % 8 != 0 || field.offset >= 32 || field.msb_right != 0 {
        return Err(invalid_layout());
    }
    // Bit offsets refer to the pixel as a native-endian 32-bit value.
    let byte = (field.offset / 8) as usize;
    Ok(Some(if cfg!(target_endian = "little") {
        byte
    } else {
        3 - byte
    }))
}

struct FramebufferLayout {
    width: usize,
    height: usize,
    stride: usize,
    start: usize,
    format: PixelFormat,
}

/// The visible part of a rectangle placed on the screen.
struct Clip {
    /// Screen position of the visible part
    x: usize,
    y: usize,
    /// Position of the visible part relative to the rectangle's origin
    source_x: usize,
    source_y: usize,
    width: usize,
    height: usize,
}

fn invalid_layout() -> RahmenError {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Invalid framebuffer layout",
    )
    .into()
}

impl FramebufferLayout {
    fn new(framebuffer: &Framebuffer) -> RahmenResult<Self> {
        let var = &framebuffer.var_screen_info;
        let stride = usize::try_from(framebuffer.fix_screen_info.line_length)
            .map_err(|_| invalid_layout())?;
        let width = usize::try_from(var.xres).map_err(|_| invalid_layout())?;
        let height = usize::try_from(var.yres).map_err(|_| invalid_layout())?;
        let virtual_width = usize::try_from(var.xres_virtual).map_err(|_| invalid_layout())?;
        let virtual_height = usize::try_from(var.yres_virtual).map_err(|_| invalid_layout())?;
        let x = usize::try_from(var.xoffset).map_err(|_| invalid_layout())?;
        let y = usize::try_from(var.yoffset).map_err(|_| invalid_layout())?;
        let virtual_row_bytes = virtual_width.checked_mul(4).ok_or_else(invalid_layout)?;
        let mapped_bytes = stride
            .checked_mul(virtual_height)
            .ok_or_else(invalid_layout)?;
        if var.bits_per_pixel != 32
            || width == 0
            || height == 0
            || x.checked_add(width).ok_or_else(invalid_layout)? > virtual_width
            || y.checked_add(height).ok_or_else(invalid_layout)? > virtual_height
            || virtual_row_bytes > stride
            || mapped_bytes > framebuffer.frame.len()
        {
            return Err(invalid_layout());
        }
        let start = y
            .checked_mul(stride)
            .and_then(|offset| x.checked_mul(4).and_then(|x| offset.checked_add(x)))
            .ok_or_else(invalid_layout)?;
        Ok(Self {
            width,
            height,
            stride,
            start,
            format: PixelFormat::new(var)?,
        })
    }

    /// Clip a rectangle of `size` at `anchor` to the visible screen area. Returns `None` if no
    /// part of it is visible.
    fn clip(&self, anchor: Vector, size: (u32, u32)) -> Option<Clip> {
        let (anchor_x, anchor_y) = (i64::from(anchor.x()), i64::from(anchor.y()));
        let x0 = anchor_x.max(0);
        let y0 = anchor_y.max(0);
        let x1 = (anchor_x + i64::from(size.0)).min(self.width as i64);
        let y1 = (anchor_y + i64::from(size.1)).min(self.height as i64);
        (x0 < x1 && y0 < y1).then(|| Clip {
            x: x0 as usize,
            y: y0 as usize,
            source_x: (x0 - anchor_x) as usize,
            source_y: (y0 - anchor_y) as usize,
            width: (x1 - x0) as usize,
            height: (y1 - y0) as usize,
        })
    }

    /// Byte offset of a visible screen position within the mapped framebuffer.
    fn offset(&self, x: usize, y: usize) -> usize {
        self.start + y * self.stride + x * 4
    }
}

impl FramebufferDisplay {
    /// Crate a new framebuffer
    pub fn new(mut framebuffer: Framebuffer) -> Self {
        assert_eq!(framebuffer.var_screen_info.bits_per_pixel, 32);
        framebuffer.frame.fill(0);
        Self { framebuffer }
    }

    /// Enter the control loop. This will periodically trigger the callback, until it returns an
    /// `Err` result.
    pub fn main_loop<F: FnMut(&mut dyn Display) -> RahmenResult<()>>(&mut self, mut callback: F) {
        while callback(self).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn write_image<P: Pixel<Subpixel = u8>>(
        &mut self,
        anchor: Vector,
        img: &ImageBuffer<P, Vec<u8>>,
    ) -> RahmenResult<()> {
        let layout = FramebufferLayout::new(&self.framebuffer)?;
        let Some(clip) = layout.clip(anchor, img.dimensions()) else {
            return Ok(());
        };
        let format = layout.format;
        let black = format.black();
        // Convert one row at a time and copy it in bulk, which suits uncached framebuffer memory.
        let channels = usize::from(P::CHANNEL_COUNT);
        let source_stride = img.width() as usize * channels;
        let mut row = vec![0; clip.width * 4];
        for y in 0..clip.height {
            let source_start = (clip.source_y + y) * source_stride + clip.source_x * channels;
            let source = &img.as_raw()[source_start..source_start + clip.width * channels];
            for (target, pixel) in row.chunks_exact_mut(4).zip(source.chunks_exact(channels)) {
                let [red, green, blue] = P::from_slice(pixel).to_rgb().0;
                target.copy_from_slice(&black);
                target[format.red] = red;
                target[format.green] = green;
                target[format.blue] = blue;
            }
            let target = layout.offset(clip.x, clip.y + y);
            self.framebuffer.frame[target..target + row.len()].copy_from_slice(&row);
        }
        Ok(())
    }
}

impl Display for FramebufferDisplay {
    fn render(&mut self, _key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Rendering {}ms", e.as_millis()));
        match img {
            DynamicImage::ImageRgb8(img) => self.write_image(anchor, img),
            DynamicImage::ImageRgba8(img) => self.write_image(anchor, img),
            img => self.write_image(anchor, &img.to_rgb8()),
        }
    }

    fn blank(&mut self, _key: usize, anchor: Vector, size: Vector) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Blanking {}ms", e.as_millis()));
        let layout = FramebufferLayout::new(&self.framebuffer)?;
        let size = (size.x().max(0) as u32, size.y().max(0) as u32);
        let Some(clip) = layout.clip(anchor, size) else {
            return Ok(());
        };
        let row = layout.format.black().repeat(clip.width);
        for y in 0..clip.height {
            let target = layout.offset(clip.x, clip.y + y);
            self.framebuffer.frame[target..target + row.len()].copy_from_slice(&row);
        }
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }

    fn update(&mut self) -> RahmenResult<()> {
        // `render` and `blank` write to the framebuffer directly.
        Ok(())
    }
}

#[cfg(test)]
mod tests;
