use std::time::Instant;

use image::{DynamicImage, GenericImageView, Pixel};

use crate::errors::RahmenResult;

pub trait Display {
    fn render<V: GenericImageView<Pixel = P>, P: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> RahmenResult<()>;

    fn dimensions(&self) -> (u32, u32);
    fn main_loop(&mut self);
}

pub fn preprocess_image(img: DynamicImage, width: u32, height: u32) -> DynamicImage {
    let start = Instant::now();
    let img = img.resize(width, height, image::imageops::FilterType::Triangle);
    println!(
        "Resize {:?}, {}ms",
        img.dimensions(),
        start.elapsed().as_millis()
    );
    img
}
