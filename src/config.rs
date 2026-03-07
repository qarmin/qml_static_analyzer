//! Configuration file parser.
//!
//! Format (TOML):
//!
//! ```toml
//! [ignore]
//! # Folders/files to skip (relative to --path).
//! paths = [
//!     "resources/keyboardLayouts",
//!     "generated/",
//! ]
//!
//! [new_child]
//! # Manually declare dynamic children that the analyzer cannot detect.
//! # Format: ParentType = ["ChildType1", "ChildType2"]
//! # The child's id is auto-derived as lowerCamelCase of ChildType.
//! # Example:
//! # RootElement = ["NewExaminationScreen"]
//! #   → adds 'newExaminationScreen' to RootElement's scope
//! #   → if NewExaminationScreen is unknown, all member access on it is allowed
//!
//! [cpp_objects]
//! # C++ objects/singletons accessible globally in QML.
//! # Format: name = "relative/path/to/header.h"
//! # Empty string means opaque — all member access is allowed without validation.
//! # When a header path is given, the analyzer parses it and validates member access.
//! diskManager = "gui/src/commons/diskManager.h"
//! guiStyle = "gui/src/commons/guistyle.h"
//! core = ""   # opaque — no header available
//!
//! [globals]
//! # Names that are always considered valid (e.g. C preprocessor macros exposed to QML).
//! names = ["AP600_GUI_TEST", "AP600_DEBUG_TIME"]
//! ```

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ignore: IgnoreSection,
    /// Extra children: parent_type_name → list of child_type_names.
    #[serde(default, rename = "new_child")]
    pub new_child: HashMap<String, Vec<String>>,
    /// C++ objects/singletons: name → header path (empty = opaque).
    #[serde(default, rename = "cpp_objects")]
    pub cpp_objects: HashMap<String, String>,
    /// Always-valid global names (e.g. preprocessor macros exposed to QML).
    #[serde(default)]
    pub globals: GlobalsSection,
}

#[derive(Debug, Default, Deserialize)]
pub struct IgnoreSection {
    /// Relative path prefixes to ignore during scanning.
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GlobalsSection {
    /// Names that are always considered valid identifiers in QML scope.
    #[serde(default)]
    pub names: Vec<String>,
}

/// Deprecated section — a flat list of C++ names (all opaque).
#[derive(Debug, Default, Deserialize)]
pub struct CppNamesSection {
    #[serde(default)]
    pub names: Vec<String>,
}

pub fn parse_config(source: &str) -> Config {
    toml::from_str(source).unwrap_or_else(|e| {
        eprintln!("warning: failed to parse config: {e}");
        Config::default()
    })
}
