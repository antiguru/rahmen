use image::DynamicImage;

use crate::errors::RahmenResult;

pub trait Display {
    fn render(&mut self, img: &DynamicImage) -> RahmenResult<()>;

    fn dimensions(&self) -> (u32, u32);
}

pub fn preprocess_image(img: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    let _t = crate::Timer::new(|e| println!("Resize {}ms", e.as_millis()));
    img.resize(width, height, image::imageops::FilterType::Triangle)
}
