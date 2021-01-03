//! Utilities to rasterize fonts to images

use crate::errors::RahmenResult;
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::hinting::HintingOptions;
use font_kit::loaders::freetype::Font;
use font_kit::source::SystemSource;

use image::{Pixel, Rgb};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I};

/// A font renderer to rasterize text to images
#[derive(Debug)]
pub struct FontRenderer {
    font: Font,
}

impl FontRenderer {
    /// Create a new font renderer, selecting a sans-serif font.
    pub fn new() -> Self {
        let font = SystemSource::new()
            .select_family_by_name("sans-serif")
            .expect("Failed to look up font")
            .fonts()[0]
            .load()
            .expect("Failed to load font");
        println!("Font: {} ({})", font.full_name(), font.family_name());
        Self { font }
    }

    /// Create a new font renderer from an given font.
    pub fn with_font(font: Font) -> Self {
        Self { font }
    }

    /// Render a text and draw pixels via a callback
    pub fn render<F: FnMut(i32, i32, &Rgb<u8>) -> RahmenResult<()>>(
        &self,
        text: &str,
        size: f32,
        dimensions: (u32, u32),
        mut draw: F,
    ) -> RahmenResult<()> {
        let hinting = HintingOptions::Full(size);
        let rasterization = RasterizationOptions::GrayscaleAa;
        let format = Format::Rgb24;

        let metrics = self.font.metrics();

        let em = size / 1.24;
        let mut base_x = 0.;
        let mut canvas = Canvas::new(Vector2I::new(dimensions.0 as _, dimensions.1 as _), format);

        for char in text.chars() {
            if let Some(glyph_id) = self.font.glyph_for_char(char) {
                let raster_rect = self
                    .font
                    .raster_bounds(
                        glyph_id,
                        size,
                        Transform2F::default(),
                        hinting,
                        rasterization,
                    )
                    .expect("Failed to determine raster bounds");
                if (raster_rect.width() + raster_rect.origin_x()) as u32 > dimensions.0 {
                    break;
                }
                self.font
                    .rasterize_glyph(
                        &mut canvas,
                        glyph_id,
                        size,
                        Transform2F::from_translation(Vector2F::new(base_x, em)),
                        hinting,
                        rasterization,
                    )
                    .expect("Font rasterization failed");

                base_x +=
                    self.font.advance(glyph_id).unwrap().x() * em / metrics.units_per_em as f32;
            }
            for y in 0..canvas.size.y() {
                let (row_start, row_end) =
                    (y as usize * canvas.stride, (y + 1) as usize * canvas.stride);
                let row = &canvas.pixels[row_start..row_end];

                for x in 0..canvas.size.x() {
                    let index = (x * format.bytes_per_pixel() as i32) as _;
                    let pixel =
                        Rgb::from_slice(&row[index..index + format.bytes_per_pixel() as usize]);
                    draw(x, y, pixel)?
                }
            }
        }
        Ok(())
    }
}
