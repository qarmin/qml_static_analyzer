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
//!
//! # Layering configs with `extends`
//!
//! A config can inherit from a base config (e.g. a checked-in framework preset under
//! `presets/`), so a new project only needs to state what's specific to it:
//!
//! ```toml
//! extends = "../presets/quickshell.toml"   # relative to this file's own directory
//!
//! [external_types]
//! names = ["MyProjectSpecificCppType"]      # merged with the preset's list, not a replacement
//! ```
//!
//! List-valued sections (`ignore.paths`, `globals.names`, `external_types.names`,
//! `roots.extra_types`) are unioned across the chain. Map-valued sections (`new_child`,
//! `cpp_objects`) are merged key-by-key, with the more specific (child) config winning on
//! conflicts. `extends` chains may nest to any depth; cycles are detected and ignored with
//! a warning.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Path (relative to this file's directory) to a base config to merge into this one.
    #[serde(default)]
    pub extends: Option<String>,
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
    /// If true, any file BFS still can't reach after the normal root pass is treated as
    /// its own root and checked anyway (still cascading into whatever it references).
    ///
    /// Some frameworks (e.g. KDE Plasma's KPackage system) load QML files directly by
    /// path convention rather than through a small set of well-known root element types —
    /// a package's `main.qml` might have almost any base type (`Item`, `PlasmoidItem`,
    /// an aliased `Kirigami.Page`, ...). Enumerating every possible root type name doesn't
    /// scale there, so this flag sidesteps type-name matching entirely.
    #[serde(default)]
    pub treat_unreachable_as_roots: bool,
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

/// Loads a config file, resolving its `extends` chain (if any).
///
/// `extends` paths are resolved relative to the directory of the config file that
/// declares them, so a preset can itself extend another preset regardless of where the
/// leaf project config lives. Cycles are detected via canonicalized paths and stop the
/// chain with a warning rather than looping forever.
pub fn load_config_file(path: &Path) -> Config {
    load_config_recursive(path, &mut Vec::new())
}

fn load_config_recursive(path: &Path, seen: &mut Vec<PathBuf>) -> Config {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if seen.contains(&canonical) {
        eprintln!("warning: circular `extends` chain detected at {}, ignoring", path.display());
        return Config::default();
    }
    seen.push(canonical);

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: cannot read config {}: {e}", path.display());
            return Config::default();
        }
    };
    let mut config = parse_config(&source);

    if let Some(extends_rel) = config.extends.take() {
        let base_path = path.parent().unwrap_or_else(|| Path::new(".")).join(&extends_rel);
        let base = load_config_recursive(&base_path, seen);
        config = merge_configs(base, config);
    }

    config
}

fn dedup_preserve_order(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|s| seen.insert(s.clone()));
}

/// Merges `base` (e.g. a framework preset) with `child` (the more specific project config).
/// Lists are unioned; maps are merged key-by-key with `child` winning on conflicting keys.
fn merge_configs(base: Config, child: Config) -> Config {
    let mut merged = base;

    merged.ignore.paths.extend(child.ignore.paths);
    dedup_preserve_order(&mut merged.ignore.paths);

    merged.new_child.extend(child.new_child);
    merged.cpp_objects.extend(child.cpp_objects);

    merged.globals.names.extend(child.globals.names);
    dedup_preserve_order(&mut merged.globals.names);

    merged.external_types.names.extend(child.external_types.names);
    dedup_preserve_order(&mut merged.external_types.names);

    merged.roots.extra_types.extend(child.roots.extra_types);
    dedup_preserve_order(&mut merged.roots.extra_types);
    merged.roots.treat_unreachable_as_roots |= child.roots.treat_unreachable_as_roots;

    merged.extends = None;
    merged
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn extends_merges_lists_and_maps_with_child_precedence() {
        let dir = std::env::temp_dir().join(format!("qml_analyzer_config_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let base_path = dir.join("base.toml");
        std::fs::File::create(&base_path)
            .unwrap()
            .write_all(
                br#"
[external_types]
names = ["ShellRoot", "Scope"]

[roots]
extra_types = ["ShellRoot"]

[cpp_objects]
core = "base/core.h"
"#,
            )
            .unwrap();

        let child_path = dir.join("child.toml");
        std::fs::File::create(&child_path)
            .unwrap()
            .write_all(
                br#"
extends = "base.toml"

[external_types]
names = ["MyType"]

[cpp_objects]
core = "child/core.h"
diskManager = "child/disk.h"
"#,
            )
            .unwrap();

        let config = load_config_file(&child_path);
        assert_eq!(config.external_types.names, vec!["ShellRoot", "Scope", "MyType"]);
        assert_eq!(config.roots.extra_types, vec!["ShellRoot"]);
        // child overrides the same key from base
        assert_eq!(config.cpp_objects.get("core"), Some(&"child/core.h".to_string()));
        assert_eq!(config.cpp_objects.get("diskManager"), Some(&"child/disk.h".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn circular_extends_is_detected_and_does_not_hang() {
        let dir = std::env::temp_dir().join(format!("qml_analyzer_config_cycle_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let a_path = dir.join("a.toml");
        let b_path = dir.join("b.toml");
        std::fs::File::create(&a_path).unwrap().write_all(b"extends = \"b.toml\"\n").unwrap();
        std::fs::File::create(&b_path).unwrap().write_all(b"extends = \"a.toml\"\n").unwrap();

        // Must terminate rather than recurse forever.
        let _ = load_config_file(&a_path);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
