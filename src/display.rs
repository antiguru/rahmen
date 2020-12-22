use image::{DynamicImage, GenericImageView, Pixel};

pub trait Display {
    type Error;

    fn render<V: GenericImageView<Pixel = P>, P: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> Result<(), Self::Error>;

    fn dimensions(&self) -> (u32, u32);
    fn main_loop(&mut self);
}

pub fn preprocess_image(img: DynamicImage, width: u32, height: u32) -> DynamicImage {
    let img = img.resize(width, height, image::imageops::FilterType::Triangle);
    println!("resized img {:?}", img.dimensions());
    img
}
