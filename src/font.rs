//! Utilities to rasterize fonts to images

use crate::errors::RahmenResult;
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::font::Font;
use font_kit::hinting::HintingOptions;

use image::DynamicImage;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use rustybuzz::{Face, UnicodeBuffer};
use std::collections::HashMap;
use std::sync::Arc;

/// A font renderer to rasterize text to images
#[derive(Debug)]
pub struct FontRenderer {
    font: Font,
    /// Font file contents and the index of `font` within them, for shaping
    data: Arc<Vec<u8>>,
    face_index: u32,
    raster_cache: HashMap<u32, RectI>,
}

impl FontRenderer {
    /// Create a new font renderer from an given font.
    pub fn with_font(font: Font) -> Self {
        let data = font.copy_font_data().expect("Failed to read font data");
        let face_index = face_index(&data, &font);
        Self {
            font,
            data,
            face_index,
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
        let face = Face::from_slice(&self.data, self.face_index)
            .expect("Failed to parse font for shaping");

        for (i, line) in text.enumerate() {
            // Shaping maps characters to glyphs, applying ligatures and kerning.
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(line);
            buffer.guess_segment_properties();
            let glyphs = rustybuzz::shape(&face, &[], buffer);

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

/// Find the face within a font file or collection that `font` was loaded from.
fn face_index(data: &[u8], font: &Font) -> u32 {
    let count = rustybuzz::ttf_parser::fonts_in_collection(data).unwrap_or(1);
    let Some(name) = font.postscript_name() else {
        return 0;
    };
    (0..count)
        .find(|&index| {
            Face::from_slice(data, index).is_some_and(|face| {
                face.names().into_iter().any(|entry| {
                    entry.name_id == rustybuzz::ttf_parser::name_id::POST_SCRIPT_NAME
                        && entry.to_string().as_deref() == Some(name.as_str())
                })
            })
        })
        .unwrap_or(0)
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
