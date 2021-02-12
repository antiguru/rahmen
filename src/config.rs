//! Configuration data for Rahmen

/// An element of the status line
#[derive(Debug, Deserialize, Clone)]
pub struct Element {
    /// Capitalize the words in the tag (optional)
    pub capitalize: Option<bool>,
    /// Collection of exif tags, ordered by priority (mandatory)
    pub exif_tags: Vec<String>,
    /// Optional regex pattern and its replacement
    pub replace: Option<Vec<Replacement>>,
    /// Optional case conversion instruction
    pub case_conversion: Option<CaseConversion>,
}

/// case conversion
#[derive(Debug, Deserialize, Clone)]
pub struct CaseConversion {
    /// from case
    pub from: String,
    /// to case
    pub to: String,
}

/// replacement regular expression and value
#[derive(Debug, Deserialize, Clone)]
pub struct Replacement {
    /// the regular expression to use
    pub regex: String,
    /// the replacement for the regex match
    pub replace: String,
}

/// Config file root structure
#[derive(Debug, Default, Deserialize, Clone)]
// this is called in rahmen.rs where a new status line formatter is constructed
pub struct Settings {
    /// Transition delay between images (optional)
    pub delay: Option<f64>,
    /// Font size of the status line (optional)
    pub font_size: Option<f32>,
    /// Status line: a collection of  elements (metadata tags, mandatory)
    pub status_line: Vec<Element>,
    /// to post-process the status line with one or more regular expressions (optional)
    pub line_replacements: Option<Vec<Replacement>>,
    /// the separator that will be inserted between every element (metadata) of the
    /// status line (optional, but default is to insert ", ")
    pub separator: Option<String>,
    /// should we return duplicate strings? if not, set to true (optional, default: true)
    pub uniquify: Option<bool>,
    /// should we hide empty fields? (optional, default: true)
    pub hide_empty: Option<bool>,
    /// python code to postprocess the status line
    pub py_code: Option<String>,
}
