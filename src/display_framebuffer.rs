//! Functionality to render images on a Linux framebuffer

use crate::display::Display;
use crate::errors::{RahmenError, RahmenResult};

use crate::Vector;
use framebuffer::Framebuffer;
use image::{Bgra, DynamicImage, GenericImage, ImageBuffer};
use std::time::Duration;

type BgraImage = ImageBuffer<Bgra<u8>, Vec<u8>>;

/// A display driver for Linux framebuffers
#[derive(Debug)]
pub struct FramebufferDisplay {
    framebuffer: Framebuffer,
    image: BgraImage,
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
        if self.image.dimensions() != self.dimensions() {
            self.image = BgraImage::from_raw(
                self.dimensions().0,
                self.dimensions().1,
                vec![0u8; (self.dimensions().0 * self.dimensions().1 * 4) as usize],
            )
            .ok_or(RahmenError::Terminate)?;
        }
        Ok(())
    }
}

impl Display for FramebufferDisplay {
    fn render(&mut self, _key: usize, anchor: Vector, img: &DynamicImage) -> RahmenResult<()> {
        self.match_dimensions()?;
        self.image
            .copy_from(&img.to_bgra8(), anchor.x() as _, anchor.y() as _)?;
        Ok(())
    }

    fn blank(&mut self, _key: usize, anchor: Vector, size: Vector) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| debug!("Blanking {}ms", e.as_millis()));
        self.match_dimensions()?;
        let black = image::FlatSamples::with_monocolor(&Bgra([0; 4]), size.x() as _, size.y() as _);
        self.image
            .copy_from(&black.as_view().unwrap(), anchor.x() as _, anchor.y() as _)?;
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }

    fn update(&mut self) -> RahmenResult<()> {
        self.framebuffer
            .frame
            .as_mut()
            .copy_from_slice(self.image.as_raw());
        Ok(())
    }
}
