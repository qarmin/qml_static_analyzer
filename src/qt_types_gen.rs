// Qt metatypes generator — converts Qt metatype JSON files to qt_types JSON.
//
// This file is compiled both as part of the main crate (for runtime `--qt-path` support)
// and included verbatim by `build.rs` (for compile-time embedding).
// It must only depend on `serde_json` and `std`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

// ── C++ → QML type mapping ─────────────────────────────────────────────────

const CPP_TO_QML: &[(&str, &str)] = &[
    ("qreal", "real"),
    ("double", "real"),
    ("float", "real"),
    ("int", "int"),
    ("uint", "int"),
    ("qint64", "int"),
    ("quint64", "int"),
    ("qintptr", "int"),
    ("bool", "bool"),
    ("QString", "string"),
    ("QUrl", "url"),
    ("QColor", "color"),
    ("QPoint", "point"),
    ("QPointF", "point"),
    ("QSize", "size"),
    ("QSizeF", "size"),
    ("QRect", "rect"),
    ("QRectF", "rect"),
    ("QFont", "font"),
    ("QMatrix4x4", "matrix4x4"),
    ("QQuaternion", "quaternion"),
    ("QVector2D", "vector2d"),
    ("QVector3D", "vector3d"),
    ("QVector4D", "vector4d"),
    ("QVariant", "var"),
    ("QJSValue", "var"),
    ("QObject", "QtObject"),
    ("void", "void"),
];

fn cpp_type_to_qml(cpp_type: &str) -> String {
    let t = cpp_type.trim().replace("const ", "").replace(" &", "").replace('&', "");
    let t = t.trim();

    for &(cpp, qml) in CPP_TO_QML {
        if t == cpp {
            return qml.to_string();
        }
    }

    // List types
    if t.starts_with("QQmlListProperty") || t.starts_with("QList<") || t.starts_with("QVector<") {
        return "list".to_string();
    }

    // Pointer — strip * and re-lookup
    if let Some(inner) = t.strip_suffix('*') {
        let inner = inner.trim();
        for &(cpp, qml) in CPP_TO_QML {
            if inner == cpp {
                return qml.to_string();
            }
        }
        if let Some(s) = inner.strip_prefix("QQuick") {
            return s.to_string();
        }
        if let Some(s) = inner.strip_prefix("QQml") {
            return s.to_string();
        }
        return inner.to_string();
    }

    // Scoped enum
    if t.contains("::") {
        return "enumeration".to_string();
    }

    "var".to_string()
}

// ── metatypes directory detection ──────────────────────────────────────────

fn find_metatypes_dir(qt_dir: &Path) -> Option<PathBuf> {
    // Qt 6.3.x installs metatypes under lib/metatypes/
    // Qt 6.8.x+ installs them directly under metatypes/
    for subdir in &["lib/metatypes", "metatypes"] {
        let p = qt_dir.join(subdir);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

// ── class loading ──────────────────────────────────────────────────────────

/// Returns `cpp_class_name → class JSON object` from all relevant metatype files.
fn load_all_classes(metatypes_dir: &Path) -> HashMap<String, Value> {
    // Module names to load, in priority order (core first for inheritance walking,
    // then QML-visible types). Missing files are silently skipped.
    // Qt 6.8 and earlier ship `qt6<name>_relwithdebinfo_metatypes.json`;
    // Qt 6.11+ ships `qt6<name>_metatypes.json` (no build-type suffix).
    const MODULES: &[&str] = &[
        "qt6core",
        "qt6gui",
        // QML engine types: Component, Binding, Connections, Timer, etc.
        // qt6qmlmeta is needed for Qt 6.8+ where Timer and others moved there.
        "qt6qml",
        "qt6qmlmeta",
        "qt6qmlmodels",
        "qt6quick",
        "qt6quicktemplates2",
        "qt6quickcontrols2",
        "qt6quickcontrols2impl",
        "qt6quicklayouts",
        "qt6quickdialogs2",
        "qt6quickdialogs2utils",
    ];

    let mut all_classes: HashMap<String, Value> = HashMap::new();

    for module in MODULES {
        // Try relwithdebinfo variant first (Qt ≤ 6.8), then plain (Qt 6.11+)
        let candidates = [
            format!("{module}_relwithdebinfo_metatypes.json"),
            format!("{module}_metatypes.json"),
        ];
        let path = candidates
            .iter()
            .map(|f| metatypes_dir.join(f))
            .find(|p| p.exists())
            .unwrap_or_else(|| metatypes_dir.join(&candidates[0]));

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(data) = serde_json::from_str::<Value>(&content) else {
            continue;
        };
        let Some(entries) = data.as_array() else { continue };

        for entry in entries {
            let Some(classes) = entry.get("classes").and_then(Value::as_array) else {
                continue;
            };
            for cls in classes {
                let name = cls
                    .get("qualifiedClassName")
                    .or_else(|| cls.get("className"))
                    .and_then(Value::as_str);
                if let Some(n) = name {
                    // First-seen wins: core types take priority
                    all_classes.entry(n.to_string()).or_insert_with(|| cls.clone());
                }
            }
        }
    }

    all_classes
}

// ── classInfos helper ─────────────────────────────────────────────────────

fn class_infos(cls: &Value) -> HashMap<String, String> {
    cls.get("classInfos")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|ci| {
                    let name = ci.get("name")?.as_str()?.to_string();
                    let value = ci.get("value")?.as_str()?.to_string();
                    Some((name, value))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── cpp→qml name mapping ──────────────────────────────────────────────────

/// Lower score = preferred when multiple C++ classes map to the same QML name.
fn prefer_score(cpp_name: &str) -> u8 {
    if cpp_name.ends_with("QmlImpl") {
        2
    } else {
        u8::from(cpp_name.ends_with("Foreign"))
    }
}

/// Returns `cpp_class_name → qml_element_name` for all classes with a QML.Element info.
fn build_cpp_to_qml(all_classes: &HashMap<String, Value>) -> HashMap<String, String> {
    let mut cpp_to_qml: HashMap<String, String> = HashMap::new();
    cpp_to_qml.insert("QObject".to_string(), "QtObject".to_string());

    // Group: qml_name → Vec<(effective_cpp_name, score)>
    let mut raw: HashMap<String, Vec<(String, u8)>> = HashMap::new();

    for (cpp_name, cls) in all_classes {
        let ci = class_infos(cls);
        let qml_name = match ci.get("QML.Element").map(String::as_str) {
            Some(n) if !n.is_empty() && n != "auto" && n != "anonymous" => n.to_string(),
            _ => continue,
        };

        // QML.Foreign: the actual properties/signals come from the foreign target class
        let effective = ci.get("QML.Foreign").cloned().unwrap_or_else(|| cpp_name.clone());

        let score = prefer_score(&effective);
        raw.entry(qml_name).or_default().push((effective, score));
    }

    for (qml_name, mut candidates) in raw {
        candidates.sort_by_key(|(_, score)| *score);
        let best_cpp = candidates.into_iter().next().expect("TODO").0;
        cpp_to_qml.insert(best_cpp, qml_name);
    }

    cpp_to_qml
}

// ── inheritance walking ────────────────────────────────────────────────────

/// Returns the nearest QML ancestor name by walking C++ superClasses.
/// Skips own_qml_name to avoid circular references.
fn find_qml_parent(
    cls: &Value,
    cpp_to_qml: &HashMap<String, String>,
    all_classes: &HashMap<String, Value>,
    own_qml_name: &str,
    visited: &mut Vec<String>,
) -> Option<String> {
    let super_classes = cls.get("superClasses").and_then(Value::as_array)?;

    for sc in super_classes {
        if sc.get("access").and_then(Value::as_str) != Some("public") {
            continue;
        }
        let Some(parent_cpp) = sc.get("name").and_then(Value::as_str) else {
            continue;
        };
        if visited.contains(&parent_cpp.to_string()) {
            continue;
        }

        if let Some(parent_qml) = cpp_to_qml.get(parent_cpp)
            && parent_qml != own_qml_name
        {
            return Some(parent_qml.clone());
        }

        if let Some(parent_cls) = all_classes.get(parent_cpp) {
            visited.push(parent_cpp.to_string());
            let result = find_qml_parent(parent_cls, cpp_to_qml, all_classes, own_qml_name, visited);
            visited.pop();
            if result.is_some() {
                return result;
            }
        }
    }

    None
}

// ── member collection ──────────────────────────────────────────────────────

/// Collects properties from `cls` and from all C++ ancestors that do NOT have
/// their own QML element name (i.e., "invisible" C++ intermediaries).
/// Stops at ancestors that map to a different QML element — those will be
/// handled via the QML-level parent chain in QtTypeDb.
fn collect_own_properties(
    cls: &Value,
    cpp_to_qml: &HashMap<String, String>,
    all_classes: &HashMap<String, Value>,
    own_qml_name: &str,
    visited: &mut Vec<String>,
) -> Map<String, Value> {
    // Start with own properties (higher priority)
    let mut result: Map<String, Value> = Map::new();
    if let Some(props) = cls.get("properties").and_then(Value::as_array) {
        for prop in props {
            let name = prop.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let type_str = prop.get("type").and_then(Value::as_str).unwrap_or("var");
            result.insert(name.to_string(), Value::String(cpp_type_to_qml(type_str)));
        }
    }

    // Walk invisible C++ parents
    let super_classes = cls
        .get("superClasses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for sc in &super_classes {
        if sc.get("access").and_then(Value::as_str) != Some("public") {
            continue;
        }
        let Some(parent_cpp) = sc.get("name").and_then(Value::as_str) else {
            continue;
        };
        if visited.contains(&parent_cpp.to_string()) {
            continue;
        }

        // Stop at ancestors that have a different QML element name
        if let Some(parent_qml) = cpp_to_qml.get(parent_cpp)
            && parent_qml != own_qml_name
        {
            continue;
        }

        if let Some(parent_cls) = all_classes.get(parent_cpp) {
            visited.push(parent_cpp.to_string());
            let parent_props = collect_own_properties(parent_cls, cpp_to_qml, all_classes, own_qml_name, visited);
            visited.pop();

            // Parent props fill in gaps (own props win)
            for (k, v) in parent_props {
                result.entry(k).or_insert(v);
            }
        }
    }

    result
}

/// Like `collect_own_properties` but for signals.
fn collect_own_signals(
    cls: &Value,
    cpp_to_qml: &HashMap<String, String>,
    all_classes: &HashMap<String, Value>,
    own_qml_name: &str,
    visited: &mut Vec<String>,
) -> Map<String, Value> {
    let mut result: Map<String, Value> = Map::new();

    if let Some(signals) = cls.get("signals").and_then(Value::as_array) {
        for sig in signals {
            let name = sig.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let args: Vec<Value> = sig
                .get("arguments")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|a| {
                            let t = a.get("type").and_then(Value::as_str).unwrap_or("var");
                            Value::String(cpp_type_to_qml(t))
                        })
                        .collect()
                })
                .unwrap_or_default();
            result.insert(name.to_string(), Value::Array(args));
        }
    }

    let super_classes = cls
        .get("superClasses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for sc in &super_classes {
        if sc.get("access").and_then(Value::as_str) != Some("public") {
            continue;
        }
        let Some(parent_cpp) = sc.get("name").and_then(Value::as_str) else {
            continue;
        };
        if visited.contains(&parent_cpp.to_string()) {
            continue;
        }

        if let Some(parent_qml) = cpp_to_qml.get(parent_cpp)
            && parent_qml != own_qml_name
        {
            continue;
        }

        if let Some(parent_cls) = all_classes.get(parent_cpp) {
            visited.push(parent_cpp.to_string());
            let parent_sigs = collect_own_signals(parent_cls, cpp_to_qml, all_classes, own_qml_name, visited);
            visited.pop();

            for (k, v) in parent_sigs {
                result.entry(k).or_insert(v);
            }
        }
    }

    result
}

fn extract_methods(cls: &Value) -> Map<String, Value> {
    let mut result: Map<String, Value> = Map::new();

    let empty = vec![];
    let methods = cls.get("methods").and_then(Value::as_array).unwrap_or(&empty);
    let slots = cls.get("slots").and_then(Value::as_array).unwrap_or(&empty);

    for method in methods.iter().chain(slots.iter()) {
        if method.get("access").and_then(Value::as_str) != Some("public") {
            continue;
        }
        if method.get("isConstructor").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let name = method.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() || name.starts_with('_') {
            continue;
        }

        let args: Vec<Value> = method
            .get("arguments")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|a| {
                        let t = a.get("type").and_then(Value::as_str).unwrap_or("var");
                        Value::String(cpp_type_to_qml(t))
                    })
                    .collect()
            })
            .unwrap_or_default();

        result.insert(name.to_string(), Value::Array(args));
    }

    result
}

fn extract_enums(cls: &Value) -> Map<String, Value> {
    let mut result: Map<String, Value> = Map::new();

    if let Some(enums) = cls.get("enums").and_then(Value::as_array) {
        for enum_entry in enums {
            let name = enum_entry.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let values: Vec<Value> = enum_entry
                .get("values")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(|s| Value::String(s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            result.insert(name.to_string(), Value::Array(values));
        }
    }

    result
}

// ── public API ────────────────────────────────────────────────────────────

/// Generate qt_types JSON string from a Qt installation directory.
///
/// `qt_dir` should be the platform-specific directory, e.g.:
///   - `/home/user/Qt/6.3.2/gcc_64`
///   - `/home/user/Qt/6.8.3/gcc_64`
pub fn generate_qt_types_json(qt_dir: &Path) -> Result<String, String> {
    let metatypes_dir = find_metatypes_dir(qt_dir)
        .ok_or_else(|| format!("Could not find metatypes directory in {}", qt_dir.display()))?;

    let all_classes = load_all_classes(&metatypes_dir);
    build_output_json(&all_classes)
}

/// Recursively finds every `*_metatypes.json` file under `dir`, regardless of module name.
///
/// Qt's own generator (`load_all_classes`) only looks at a fixed list of Qt module names
/// in a single flat directory. Third-party C++/QML modules (Quickshell, KDE Frameworks,
/// Plasma, ...) emit metatype JSON in the exact same moc/AUTOMOC format when built with
/// CMake, just under module-specific names and often nested build directories — so this
/// walks the whole tree instead of assuming a fixed layout.
fn find_all_metatypes_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            result.extend(find_all_metatypes_files(&path));
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with("_metatypes.json")) {
            result.push(path);
        }
    }
    result
}

/// Loads classes from every `*_metatypes.json` found under any of `dirs`. First-seen wins
/// on name clashes, same as `load_all_classes`.
fn load_extra_classes(dirs: &[PathBuf]) -> HashMap<String, Value> {
    let mut all_classes: HashMap<String, Value> = HashMap::new();
    for dir in dirs {
        for path in find_all_metatypes_files(dir) {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(data) = serde_json::from_str::<Value>(&content) else {
                continue;
            };
            let Some(entries) = data.as_array() else { continue };
            for entry in entries {
                let Some(classes) = entry.get("classes").and_then(Value::as_array) else {
                    continue;
                };
                for cls in classes {
                    let name = cls
                        .get("qualifiedClassName")
                        .or_else(|| cls.get("className"))
                        .and_then(Value::as_str);
                    if let Some(n) = name {
                        all_classes.entry(n.to_string()).or_insert_with(|| cls.clone());
                    }
                }
            }
        }
    }
    all_classes
}

/// Same as [`generate_qt_types_json`], but also merges in classes from arbitrary
/// C++/QML module build directories (e.g. a Quickshell or KDE Frameworks build tree),
/// found by recursively scanning `extra_dirs` for any `*_metatypes.json` file.
///
/// Qt's own classes are loaded first and take priority on name clashes, since they're
/// needed as the inheritance root for third-party types (e.g. a Quickshell `PanelWindow`
/// extending `QQuickItem`).
pub fn generate_module_types_json(qt_dir: &Path, extra_dirs: &[PathBuf]) -> Result<String, String> {
    let metatypes_dir = find_metatypes_dir(qt_dir)
        .ok_or_else(|| format!("Could not find metatypes directory in {}", qt_dir.display()))?;

    let mut all_classes = load_all_classes(&metatypes_dir);
    for (name, cls) in load_extra_classes(extra_dirs) {
        all_classes.entry(name).or_insert(cls);
    }
    build_output_json(&all_classes)
}

fn build_output_json(all_classes: &HashMap<String, Value>) -> Result<String, String> {
    let cpp_to_qml = build_cpp_to_qml(all_classes);

    let mut output: Map<String, Value> = Map::new();
    output.insert(
        "_comment".to_string(),
        Value::String(
            "Auto-generated from Qt metatype JSON files. Do not edit manually — \
             re-run the generator instead."
                .to_string(),
        ),
    );

    // QtObject is the root of all QML types
    output.insert(
        "QtObject".to_string(),
        json!({
            "parent": null,
            "properties": { "objectName": "string" },
            "signals": { "objectNameChanged": ["string"] },
            "methods": { "destroy": [], "toString": [] },
            "enums": {}
        }),
    );

    // Collect all QML types, sorted by name for determinism
    let mut qml_entries: Vec<(String, String)> = cpp_to_qml
        .iter()
        .filter(|(_, qml)| *qml != "QtObject")
        .map(|(cpp, qml)| (cpp.clone(), qml.clone()))
        .collect();
    qml_entries.sort_by(|a, b| a.1.cmp(&b.1));

    for (cpp_name, qml_name) in &qml_entries {
        let Some(cls) = all_classes.get(cpp_name) else {
            // No class data — minimal entry
            output.insert(
                qml_name.clone(),
                json!({
                    "parent": null,
                    "properties": {},
                    "signals": {},
                    "methods": {},
                    "enums": {}
                }),
            );
            continue;
        };

        let qml_parent = find_qml_parent(cls, &cpp_to_qml, all_classes, qml_name, &mut vec![]);

        let mut properties = collect_own_properties(cls, &cpp_to_qml, all_classes, qml_name, &mut vec![]);

        let signals = collect_own_signals(cls, &cpp_to_qml, all_classes, qml_name, &mut vec![]);

        // QML.Attached: merge attached type's properties (e.g. ListView.isCurrentItem)
        let ci = class_infos(cls);
        if let Some(attached_cpp) = ci.get("QML.Attached")
            && let Some(att_cls) = all_classes.get(attached_cpp)
        {
            let att_props = collect_own_properties(att_cls, &cpp_to_qml, all_classes, qml_name, &mut vec![]);
            for (k, v) in att_props {
                properties.entry(k).or_insert(v);
            }
        }

        let methods = extract_methods(cls);
        let enums = extract_enums(cls);

        output.insert(
            qml_name.clone(),
            json!({
                "parent": qml_parent,
                "properties": properties,
                "signals": signals,
                "methods": methods,
                "enums": enums,
            }),
        );
    }

    // Post-processing: inject well-known grouped properties that aren't
    // directly visible in the metatypes.
    if let Some(item) = output.get_mut("Item")
        && let Some(props) = item.get_mut("properties").and_then(Value::as_object_mut)
    {
        props
            .entry("anchors".to_string())
            .or_insert(Value::String("Anchors".to_string()));
    }

    if let Some(rect) = output.get_mut("Rectangle")
        && let Some(props) = rect.get_mut("properties").and_then(Value::as_object_mut)
    {
        props
            .entry("border".to_string())
            .or_insert(Value::String("BorderPen".to_string()));
    }

    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_json(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::File::create(path).unwrap().write_all(content.as_bytes()).unwrap();
    }

    /// Builds a minimal fake Qt install (just enough `QObject`/`QQuickItem` metatype data
    /// for inheritance walking) plus a fake third-party module build dir, then verifies
    /// `generate_module_types_json` merges the two into a single, inheritance-aware DB —
    /// without needing a real Qt SDK on the test machine.
    #[test]
    fn extra_metatypes_dir_merges_third_party_module_with_qt_inheritance() {
        let root = std::env::temp_dir().join(format!("qml_analyzer_qt_gen_test_{}", std::process::id()));
        let qt_dir = root.join("qt");
        let extra_dir = root.join("thirdparty_build");

        write_json(
            &qt_dir.join("metatypes/qt6core_metatypes.json"),
            r#"[{"classes": [{"className": "QObject", "qualifiedClassName": "QObject", "classInfos": [], "properties": [], "signals": [], "methods": []}]}]"#,
        );
        write_json(
            &qt_dir.join("metatypes/qt6quick_metatypes.json"),
            r#"[{"classes": [{
                "className": "QQuickItem",
                "qualifiedClassName": "QQuickItem",
                "superClasses": [{"access": "public", "name": "QObject"}],
                "classInfos": [{"name": "QML.Element", "value": "Item"}],
                "properties": [{"name": "width", "type": "qreal", "index": 0}],
                "signals": [],
                "methods": []
            }]}]"#,
        );
        write_json(
            &extra_dir.join("build/fakemodule_metatypes.json"),
            r#"[{"classes": [{
                "className": "FakePanel",
                "qualifiedClassName": "FakePanel",
                "superClasses": [{"access": "public", "name": "QQuickItem"}],
                "classInfos": [{"name": "QML.Element", "value": "FakePanel"}],
                "properties": [{"name": "screenName", "type": "QString", "index": 0}],
                "signals": [],
                "methods": []
            }]}]"#,
        );

        let json = generate_module_types_json(&qt_dir, &[extra_dir]).expect("generation should succeed");
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let fake_panel = parsed.get("FakePanel").expect("FakePanel must be present in merged output");
        assert_eq!(fake_panel["parent"], "Item", "FakePanel must resolve its Qt ancestor through the merged DB");
        assert!(
            fake_panel["properties"]["screenName"] == "string",
            "FakePanel's own property must be preserved: {fake_panel}"
        );
        assert!(parsed.get("Item").is_some(), "Qt's own Item type must still be present");

        let _ = std::fs::remove_dir_all(&root);
    }
}
