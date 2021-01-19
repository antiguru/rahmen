//! Configuration data for Rahmen

/// An element of the status line
#[derive(Debug, Deserialize, Clone)]
pub struct Element {
    /// Capitalize the words in the tag
    pub capitalize: Option<bool>,
    /// Collection of exif tags, ordered by priority
    pub exif_tags: Vec<String>,
    /// Optional regex pattern and its replacement
    pub replace: Option<Vec<Replacement>>,
}

/// replacement regular expression and value
#[derive(Debug, Deserialize, Clone)]
pub struct Replacement {
    /// the regular expression to use
    pub regex: Option<String>,
    /// the replacement for the regex match
    pub replace: Option<String>,
}

/// Config file root structure
#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    /// Transition delay between images
    pub delay: Option<f64>,
    /// Font size of the status line
    pub font_size: Option<f32>,
    /// Status line elements
    pub status_line: Vec<Element>,
}
