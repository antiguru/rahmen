use font_kit::family_name::FamilyName;
use font_kit::font::Font;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;

pub(super) fn load(path: Option<&str>) -> Font {
    match path {
        Some(path) => Font::from_path(path, 0).expect("Failed to load requested font"),
        None => SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .expect("No system sans-serif font found")
            .load()
            .expect("Failed to load system sans-serif font"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_has_renderable_text() {
        let font = load(None);
        assert!(font.glyph_for_char('M').is_some());
    }

    #[test]
    #[should_panic(expected = "Failed to load requested font")]
    fn invalid_explicit_path_does_not_fall_back() {
        load(Some("/rahmen-nonexistent-font-directory/font.ttf"));
    }
}
