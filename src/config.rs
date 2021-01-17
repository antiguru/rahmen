//! Configuration data for Rahmen

/// An element of the status line
#[derive(Debug, Deserialize, Clone)]
pub struct Element {
    /// Capitalize the words in the tag
    pub capitalize: Option<bool>,
    /// Collection of exif tags, ordered by priority
    pub exif_tags: Vec<String>,
    /// Optional regular expression to extract data. Requires replacement pattern
    pub regex: Option<String>,
    /// Optional replacement pattern. Requires regex
    pub replace: Option<String>,
}

/// Config file root structure
#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    /// Transition delay between images
    pub delay: Option<f64>,
    /// Status line elements
    pub status_line: Vec<Element>,
}
