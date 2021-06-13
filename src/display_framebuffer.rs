//! Functionality to render images on a Linux framebuffer

use crate::display::Display;
use crate::errors::RahmenResult;

use framebuffer::Framebuffer;
use image::{Bgra, DynamicImage, FlatSamples, GenericImage, GenericImageView, Pixel};
use std::time::Duration;

/// A display driver for Linux framebuffers
#[derive(Debug)]
pub struct FramebufferDisplay {
    framebuffer: Framebuffer,
}

impl FramebufferDisplay {
    /// Crate a new framebuffer
    pub fn new(framebuffer: Framebuffer) -> Self {
        assert_eq!(framebuffer.var_screen_info.bits_per_pixel, 32);
        Self { framebuffer }
    }

    /// Enter the control loop. This will periodically trigger the callback, until it returns an
    /// `Err` result.
    pub fn main_loop<F: FnMut(&mut dyn Display) -> RahmenResult<()>>(&mut self, mut callback: F) {
        while callback(self).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn image_buffer(&mut self) -> image::ImageBuffer<Bgra<u8>, &mut [u8]> {
        image::ImageBuffer::<Bgra<_>, _>::from_raw(
            self.dimensions().0,
            self.dimensions().1,
            &mut *self.framebuffer.frame,
        )
        .unwrap()
    }
}

impl Display for FramebufferDisplay {
    fn render(
        &mut self,
        _key: usize,
        x_offset: u32,
        y_offset: u32,
        img: &DynamicImage,
    ) -> RahmenResult<()> {
        let mut buffer = self.image_buffer();
        for (x, y, pixel) in img.pixels() {
            *buffer.get_pixel_mut(x_offset + x, y_offset + y) = pixel.to_bgra();
        }
        Ok(())
    }

    fn blank(
        &mut self,
        _key: usize,
        x_offset: u32,
        y_offset: u32,
        x_size: u32,
        y_size: u32,
    ) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| println!("Blanking {}ms", e.as_millis()));
        let mut buffer = self.image_buffer();
        let black = FlatSamples::with_monocolor(&Bgra([0; 4]), x_size, y_size);
        buffer.copy_from(&black.as_view().unwrap(), x_offset, y_offset)?;
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }

    fn update(&mut self) -> RahmenResult<()> {
        Ok(())
    }
}
