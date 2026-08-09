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
//!
//! [external_types]
//! # Type names provided by a compiled C++/QML module the analyzer cannot introspect
//! # (e.g. Quickshell's `ShellRoot`, `PanelWindow`, `Scope`, or KDE's `PlasmoidItem`).
//! # Instantiating them (`PanelWindow { ... }`) no longer raises "Unknown type", and
//! # property assignments directly on them are not validated (own members unknown).
//! # Their children ARE still checked normally — only the element itself is opaque.
//! names = ["ShellRoot", "PanelWindow", "Scope", "PlasmoidItem"]
//!
//! [roots]
//! # Extra root element types (besides Window/ApplicationWindow/Dialog) from which the
//! # BFS reachability scan starts. Needed for frameworks whose entry point is not a
//! # standard Qt Quick window type (e.g. Quickshell's `ShellRoot`/`Scope`).
//! extra_types = ["ShellRoot", "Scope"]
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
    /// Type names from compiled C++/QML modules that cannot be introspected.
    #[serde(default, rename = "external_types")]
    pub external_types: ExternalTypesSection,
    /// Extra BFS root element types, on top of the built-in Window/ApplicationWindow/Dialog.
    #[serde(default)]
    pub roots: RootsSection,
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

#[derive(Debug, Default, Deserialize)]
pub struct ExternalTypesSection {
    /// Element type names treated as known-but-opaque (unqualified, e.g. "PanelWindow").
    #[serde(default)]
    pub names: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RootsSection {
    /// Extra element type names that count as BFS reachability roots.
    #[serde(default)]
    pub extra_types: Vec<String>,
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
