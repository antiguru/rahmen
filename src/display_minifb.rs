use crate::display::{preprocess_image, Display};
use crate::errors::{ProviderError, RahmenError, RahmenResult};
use crate::image::{GenericImageView, Pixel};
use crate::provider::Provider;

use image::DynamicImage;
use minifb::{Key, Window};

pub struct MiniFBDisplay<P: Provider<DynamicImage>> {
    window: Window,
    provider: P,
}

impl<P: Provider<DynamicImage>> MiniFBDisplay<P> {
    pub fn new(window: Window, provider: P) -> Self {
        Self { window, provider }
    }
}

fn from_rgb(pixel: &image::Rgb<u8>) -> u32 {
    let (r, g, b, _a) = pixel.channels4();
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

impl<P: Provider<DynamicImage>> Display for MiniFBDisplay<P> {
    fn render<V: GenericImageView<Pixel = Pi>, Pi: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> RahmenResult<()> {
        let mut buffer = vec![0; (self.dimensions().0 * self.dimensions().1) as _];
        let x_offset = (self.dimensions().0 - img.dimensions().0) / 2;
        let y_offset = (self.dimensions().1 - img.dimensions().1) / 2;
        for (x, y, pixel) in img.pixels() {
            buffer[(x_offset + x + self.dimensions().0 * (y + y_offset)) as usize] =
                from_rgb(&pixel.to_rgb());
        }
        self.window
            .update_with_buffer(&buffer, self.dimensions().0 as _, self.dimensions().1 as _)
            .map_err(RahmenError::MiniFBError)
    }

    fn dimensions(&self) -> (u32, u32) {
        let (x, y) = self.window.get_size();
        (x as _, y as _)
    }

    fn main_loop(&mut self) {
        while self.window.is_open() && !self.window.is_key_down(Key::Escape) {
            self.window.update();
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
        }
    }
}
