//! Functionality to render images on a Linux framebuffer

use crate::display::Display;
use crate::errors::RahmenResult;

use framebuffer::Framebuffer;
use image::{DynamicImage, GenericImageView, Pixel};
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
    pub fn main_loop<F: FnMut(Box<&mut dyn Display>) -> RahmenResult<()>>(
        &mut self,
        mut callback: F,
    ) {
        while callback(Box::new(self)).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Display for FramebufferDisplay {
    fn render(&mut self, img: &DynamicImage) -> RahmenResult<()> {
        let mut buffer = image::ImageBuffer::<image::Bgra<_>, _>::from_raw(
            self.dimensions().0,
            self.dimensions().1,
            &mut *self.framebuffer.frame,
        )
        .unwrap();
        for (buffer_pixel, (_, _, pixel)) in buffer.pixels_mut().zip(img.pixels()) {
            *buffer_pixel = pixel.to_bgra();
        }
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }
}
