use crate::display::{Display};
use crate::errors::{RahmenResult};

use framebuffer::Framebuffer;
use image::{GenericImageView, Pixel};


pub fn setup_framebuffer(framebuffer: &mut Framebuffer) {
    assert_eq!(framebuffer.var_screen_info.bits_per_pixel, 32);
}

pub struct FramebufferDisplay {
    framebuffer: Framebuffer,
    buffer: Vec<u8>,
}

impl FramebufferDisplay {
    pub fn new(framebuffer: Framebuffer) -> Self {
        Self {
            buffer: vec![
                0;
                (framebuffer.var_screen_info.xres
                    * framebuffer.var_screen_info.yres
                    * framebuffer.var_screen_info.bits_per_pixel
                    / 8) as _
            ],
            framebuffer,
        }
    }
}

impl Display for FramebufferDisplay {
    fn render<V: GenericImageView<Pixel = Pi>, Pi: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> RahmenResult<()> {
        let _t = crate::Timer::new(|e| println!("Rendering {}ms", e.as_millis()));
        println!("Image dimensions: {:?}", img.dimensions());
        self.buffer.clear();
        self.buffer
            .extend(std::iter::repeat(0).take(self.buffer.capacity()));
        let dimensions = self.dimensions();
        let x_offset = (dimensions.0 - img.dimensions().0) / 2;
        let y_offset = (dimensions.1 - img.dimensions().1) / 2;
        for (x, y, pixel) in img.pixels() {
            let index = (x_offset + x + dimensions.0 * (y + y_offset)) as usize * 4;
            self.buffer[index..index + 3].copy_from_slice(pixel.to_bgr().channels());
        }
        self.framebuffer.frame[..self.buffer.len()].copy_from_slice(&self.buffer[..]);
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }
}
