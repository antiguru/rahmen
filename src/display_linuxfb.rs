use crate::display::{preprocess_image, Display};
use crate::errors::{ProviderError, RahmenError};
use crate::provider::Provider;
use image::{GenericImageView, Pixel};
use linuxfb::Framebuffer;
use memmap::MmapMut;
use std::time::{Duration, Instant};

pub fn setup_framebuffer(framebuffer: &mut Framebuffer) {
    framebuffer.set_bytes_per_pixel(4).unwrap();
    framebuffer.set_offset(0, 0).unwrap();

    assert_eq!(framebuffer.get_bytes_per_pixel(), 4);
}

pub struct LinuxFBDisplay<P: Provider> {
    provider: P,
    framebuffer: Framebuffer,
    map: MmapMut,
    buffer: Vec<u8>,
}

impl<P: Provider> LinuxFBDisplay<P> {
    pub fn new(provider: P, framebuffer: Framebuffer) -> Self {
        Self {
            map: framebuffer.map().expect("Failed to map framebuffer"),
            buffer: vec![
                0;
                (framebuffer.get_size().0
                    * framebuffer.get_size().1
                    * framebuffer.get_bytes_per_pixel()) as _
            ],
            provider,
            framebuffer,
        }
    }
}

impl<P: Provider> Display for LinuxFBDisplay<P> {
    type Error = linuxfb::Error;

    fn render<V: GenericImageView<Pixel = Pi>, Pi: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> Result<(), Self::Error> {
        let start = Instant::now();
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
        self.map[..].copy_from_slice(&self.buffer[..]);
        println!("Rendering: {}ms", start.elapsed().as_millis());
        Ok(())
    }

    fn dimensions(&self) -> (u32, u32) {
        self.framebuffer.get_size()
    }

    fn main_loop(&mut self) {
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
                Err(RahmenError::Provider(ProviderError::Idle)) => { /* continue */ }
                Err(e) => panic!("Failed to load image: {}", e),
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
