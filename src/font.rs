//! Utilities to rasterize fonts to images

use crate::errors::RahmenResult;
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::font::Font;
use font_kit::hinting::HintingOptions;

use image::DynamicImage;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use std::collections::HashMap;

/// A font renderer to rasterize text to images
#[derive(Debug)]
pub struct FontRenderer {
    font: Font,
    raster_cache: HashMap<char, Option<(u32, RectI)>>,
}

impl FontRenderer {
    /// Create a new font renderer from an given font.
    pub fn with_font(font: Font) -> Self {
        Self {
            font,
            raster_cache: HashMap::new(),
        }
    }

    /// Render a text and return an image containing the rasterized text
    pub fn render<'a, I: Iterator<Item = &'a str>>(
        &mut self,
        text: I,
        size: f32,
        dimensions: (u32, u32),
    ) -> RahmenResult<DynamicImage> {
        let hinting = HintingOptions::Full(size);
        let rasterization = RasterizationOptions::GrayscaleAa;
        // CoreText overwrites the canvas when converting each glyph to RGB24.
        // Render into its native format and convert once after all glyphs.
        let format = if cfg!(any(target_os = "macos", target_os = "ios")) {
            Format::Rgba32
        } else {
            Format::Rgb24
        };

        let metrics = self.font.metrics();
        // width of character, larger values of optional factor (eg size * 1.1) increase spacing
        let em = size * 1.05;
        // dimensions are set in dataflow.rs
        let mut canvas = Canvas::new(Vector2I::new(dimensions.0 as _, dimensions.1 as _), format);
        let font = &self.font;

        for (i, line) in text.enumerate() {
            // horizontal offset from the start of the text
            let mut base_x = 0.;
            for char in line.chars() {
                if let Some((glyph_id, raster_rect)) =
                    self.raster_cache.entry(char).or_insert_with(|| {
                        font.glyph_for_char(char).map(|glyph_id| {
                            (
                                glyph_id,
                                font.raster_bounds(
                                    glyph_id,
                                    size,
                                    Transform2F::default(),
                                    hinting,
                                    rasterization,
                                )
                                .expect("Failed to determine raster bounds"),
                            )
                        })
                    })
                {
                    if (base_x as i32 + raster_rect.width() + raster_rect.origin_x()) as u32
                        > dimensions.0
                    {
                        break;
                    }
                    self.font
                        .rasterize_glyph(
                            &mut canvas,
                            *glyph_id,
                            size,
                            Transform2F::from_translation(Vector2F::new(
                                base_x,
                                i as f32 * em + size,
                            )),
                            hinting,
                            rasterization,
                        )
                        .expect("Font rasterization failed");

                    base_x += self.font.advance(*glyph_id).unwrap().x() * em
                        / metrics.units_per_em as f32;
                }
            }
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let image = DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(canvas.size.x() as _, canvas.size.y() as _, canvas.pixels)
                .unwrap(),
        )
        .to_rgb8();
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let image =
            image::RgbImage::from_raw(canvas.size.x() as _, canvas.size.y() as _, canvas.pixels)
                .unwrap();
        Ok(DynamicImage::ImageRgb8(image))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use font_kit::{family_name::FamilyName, properties::Properties, source::SystemSource};

    #[test]
    fn rendering_preserves_previous_lines() {
        let font = SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .unwrap()
            .load()
            .unwrap();
        let mut renderer = FontRenderer::with_font(font);
        let two_lines = renderer
            .render(["M", "M"].into_iter(), 16., (64, 64))
            .unwrap();
        let last_line = renderer
            .render(["", "M"].into_iter(), 16., (64, 64))
            .unwrap();
        assert!(two_lines.to_rgb8() != last_line.to_rgb8());
    }
}
