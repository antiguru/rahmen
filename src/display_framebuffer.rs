use crate::display::{preprocess_image, Display};
use crate::errors::{ProviderError, RahmenError, RahmenResult};
use crate::provider::Provider;
use framebuffer::Framebuffer;
use image::{GenericImageView, Pixel};
use std::time::Duration;

pub fn setup_framebuffer(framebuffer: &mut Framebuffer) {
    assert_eq!(framebuffer.var_screen_info.bits_per_pixel, 32);
}

pub struct FramebufferDisplay<P: Provider> {
    provider: P,
    framebuffer: Framebuffer,
    buffer: Vec<u8>,
}

impl<P: Provider> FramebufferDisplay<P> {
    pub fn new(provider: P, framebuffer: Framebuffer) -> Self {
        Self {
            buffer: vec![
                0;
                (framebuffer.var_screen_info.xres
                    * framebuffer.var_screen_info.yres
                    * framebuffer.var_screen_info.bits_per_pixel
                    / 8) as _
            ],
            provider,
            framebuffer,
        }
    }
}

impl<P: Provider> Display for FramebufferDisplay<P> {
    fn render<V: GenericImageView<Pixel = Pi>, Pi: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> RahmenResult<()> {
        println!("Clearing buffer");
        self.buffer.clear();
        println!("zeroing buffer");
        self.buffer
            .extend(std::iter::repeat(0).take(self.buffer.capacity()));
        let dimensions = self.dimensions();
        let x_offset = (dimensions.0 - img.dimensions().0) / 2;
        let y_offset = (dimensions.1 - img.dimensions().1) / 2;
        println!("x_offset: {}, y_offset: {}", x_offset, y_offset);
        println!("writing image to buffer");
        for (x, y, pixel) in img.pixels() {
            let index = (x_offset + x + dimensions.0 * (y + y_offset)) as usize * 4;
            self.buffer[index..index + 3].copy_from_slice(pixel.to_bgr().channels());
        }
        println!("writing buffer to framebuffer");
        self.framebuffer.frame[..self.buffer.len()].copy_from_slice(&self.buffer[..]);
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        (
            self.framebuffer.var_screen_info.xres,
            self.framebuffer.var_screen_info.yres,
        )
    }

    fn main_loop(&mut self) {
        println!("main_loop, dimesions: {:?}", self.dimensions());
        loop {
            match self.provider.next_image() {
                Ok(img) => self
                    .render(preprocess_image(
                        img,
                        self.dimensions().0,
                        self.dimensions().1,
                    ))
                    .unwrap(),
                Err(RahmenError::Provider(ProviderError::Eof)) => break,
                Err(RahmenError::Provider(ProviderError::Idle)) => continue,
                Err(e) => panic!("Failed to load image: {}", e),
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
