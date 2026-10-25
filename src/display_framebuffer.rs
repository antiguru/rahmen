//! Functionality to render images on a Linux framebuffer

use crate::display::Display;
use crate::errors::{RahmenError, RahmenResult};

use crate::Vector;
use framebuffer::Framebuffer;
use image::{DynamicImage, GenericImage, Rgba, RgbaImage};
use std::time::Duration;

/// A display driver for Linux framebuffers.
///
/// The backing buffer is an [`RgbaImage`] whose bytes are kept in the framebuffer's native
/// BGRA channel order (the dedicated `Bgra` pixel type was removed from the `image` crate).
#[derive(Debug)]
pub struct FramebufferDisplay {
    framebuffer: Framebuffer,
    image: RgbaImage,
}

struct FramebufferLayout {
    row_bytes: usize,
    height: usize,
    stride: usize,
    start: usize,
    image_bytes: usize,
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
        let row_bytes = width.checked_mul(4).ok_or_else(invalid_layout)?;
        let image_bytes = row_bytes.checked_mul(height).ok_or_else(invalid_layout)?;
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
            row_bytes,
            height,
            stride,
            start,
            image_bytes,
        })
    }
}

impl FramebufferDisplay {
    /// Crate a new framebuffer
    pub fn new(mut framebuffer: Framebuffer) -> Self {
        assert_eq!(framebuffer.var_screen_info.bits_per_pixel, 32);
        framebuffer.frame.fill(0);
        Self {
            framebuffer,
            image: Default::default(),
        }
    }

    /// Enter the control loop. This will periodically trigger the callback, until it returns an
    /// `Err` result.
    pub fn main_loop<F: FnMut(&mut dyn Display) -> RahmenResult<()>>(&mut self, mut callback: F) {
        while callback(self).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn match_dimensions(&mut self) -> RahmenResult<()> {
        let layout = FramebufferLayout::new(&self.framebuffer)?;
        if self.image.dimensions() != self.dimensions() {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(layout.image_bytes)
                .map_err(|_| invalid_layout())?;
            pixels.resize(layout.image_bytes, 0);
            self.image = RgbaImage::from_raw(self.dimensions().0, self.dimensions().1, pixels)
                .ok_or_else(invalid_layout)?;
        }
        Ok(())
    }
}

impl Display for FramebufferDisplay {
    fn render(&mut self, _key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()> {
        self.match_dimensions()?;
        // Convert to RGBA and swap the red/blue channels to obtain the framebuffer's BGRA order.
        let mut bgra = img.to_rgba8();
        for pixel in bgra.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        self.image
            .copy_from(&bgra, anchor.x() as _, anchor.y() as _)?;
        Ok(())
    }

    fn blank(&mut self, _key: usize, anchor: Vector, size: Vector) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Blanking {}ms", e.as_millis()));
        self.match_dimensions()?;
        let black =
            image::FlatSamples::with_monocolor(&Rgba([0u8; 4]), size.x() as _, size.y() as _);
        self.image.copy_from(
            &black.as_view::<Rgba<u8>>().unwrap(),
            anchor.x() as _,
            anchor.y() as _,
        )?;
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }

    fn update(&mut self) -> RahmenResult<()> {
        let layout = FramebufferLayout::new(&self.framebuffer)?;
        if self.image.dimensions() != self.dimensions()
            || self.image.as_raw().len() != layout.image_bytes
        {
            return Err(invalid_layout());
        }
        // The validated virtual geometry bounds every row before any bytes are written.
        for row in 0..layout.height {
            let source = row * layout.row_bytes;
            let target = layout.start + row * layout.stride;
            self.framebuffer.frame[target..target + layout.row_bytes]
                .copy_from_slice(&self.image.as_raw()[source..source + layout.row_bytes]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
