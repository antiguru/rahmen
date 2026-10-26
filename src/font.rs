//! Utilities to rasterize fonts to images

use crate::errors::RahmenResult;
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::font::Font;
use font_kit::hinting::HintingOptions;

use harfrust::font::Blob;
use harfrust::font::name::NameId;
use harfrust::{Buffer, ShapeOptions, ShaperFont};
use image::DynamicImage;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// A font renderer to rasterize text to images
pub struct FontRenderer {
    font: Font,
    /// The same font, parsed for shaping
    shaping_font: harfrust::Font,
    raster_cache: HashMap<u32, RectI>,
}

impl fmt::Debug for FontRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontRenderer")
            .field("font", &self.font)
            .finish_non_exhaustive()
    }
}

impl FontRenderer {
    /// Create a new font renderer from an given font.
    pub fn with_font(font: Font) -> Self {
        let shaping_font = shaping_font(&font);
        Self {
            font,
            shaping_font,
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
        // font units to pixels
        let scale = size / metrics.units_per_em as f32;
        // vertical distance between lines
        let line_height = size * 1.05;
        // dimensions are set in dataflow.rs
        let mut canvas = Canvas::new(Vector2I::new(dimensions.0 as _, dimensions.1 as _), format);
        let font = &self.font;
        let shaper = ShaperFont::new(&self.shaping_font);

        for (i, line) in text.enumerate() {
            // Shaping maps characters to glyphs, applying ligatures and kerning.
            let mut glyphs = Buffer::new();
            glyphs.push_str(line);
            glyphs.guess_segment_properties();
            harfrust::shape(&shaper, &mut glyphs, ShapeOptions::new())
                .expect("Text shaping failed");

            // horizontal offset from the start of the text
            let mut base_x = 0.;
            let baseline = i as f32 * line_height + size;
            for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
                let glyph_id = info.glyph_id;
                let x = base_x + position.x_offset as f32 * scale;
                // Font units point up, canvas coordinates point down.
                let y = baseline - position.y_offset as f32 * scale;
                base_x += position.x_advance as f32 * scale;
                let raster_rect = self.raster_cache.entry(glyph_id).or_insert_with(|| {
                    font.raster_bounds(
                        glyph_id,
                        size,
                        Transform2F::default(),
                        hinting,
                        rasterization,
                    )
                    .expect("Failed to determine raster bounds")
                });
                if (x as i32 + raster_rect.width() + raster_rect.origin_x()) as u32 > dimensions.0 {
                    break;
                }
                font.rasterize_glyph(
                    &mut canvas,
                    glyph_id,
                    size,
                    Transform2F::from_translation(Vector2F::new(x, y)),
                    hinting,
                    rasterization,
                )
                .expect("Font rasterization failed");
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

/// Parse the font file again for shaping, picking the face `font` was loaded from in a collection.
fn shaping_font(font: &Font) -> harfrust::Font {
    let data: Arc<dyn AsRef<[u8]> + Send + Sync> =
        font.copy_font_data().expect("Failed to read font data");
    let data = Blob::from(data);
    let name = font.postscript_name();
    let faces: Vec<_> = (0..)
        .map_while(|index| harfrust::Font::new(data.clone(), index))
        .collect();
    let matches_name = |face: &&harfrust::Font| {
        let postscript = face.names().get(NameId::POSTSCRIPT_NAME);
        let postscript = postscript.as_ref().and_then(|name| name.english_or_first());
        match (postscript, &name) {
            (Some(postscript), Some(name)) => postscript.name() == name.as_str(),
            _ => false,
        }
    };
    let face = faces.iter().find(matches_name).or(faces.first());
    face.cloned().expect("Failed to parse font for shaping")
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

    #[test]
    fn shaping_applies_ligatures() {
        let font = SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .unwrap()
            .load()
            .unwrap();
        let mut renderer = FontRenderer::with_font(font);
        let ligature = renderer.render(["ff"].into_iter(), 32., (64, 48)).unwrap();
        // A zero-width non-joiner keeps the letters separate.
        let separate = renderer
            .render(["f\u{200C}f"].into_iter(), 32., (64, 48))
            .unwrap();
        assert!(ligature.to_rgb8() != separate.to_rgb8());
    }
}
