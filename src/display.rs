use image::{GenericImageView, Pixel};

pub trait Display {
    type Error;

    fn render<V: GenericImageView<Pixel = P>, P: Pixel<Subpixel = u8>>(
        &mut self,
        img: V,
    ) -> Result<(), Self::Error>;

    fn dimensions(&self) -> (u32, u32);
    fn main_loop(&mut self);
}
