use std::collections::HashMap;

use crate::types::PropertyType;

// ── pomocnicze ────────────────────────────────────────────────────────────

/// Extract module aliases from a file's import list.
///
/// Returns a map `alias → is_qt_module` where `is_qt_module` is `true` when
/// the imported module name starts with `Qt` (e.g. `QtQuick.Controls`).
/// Non-Qt aliases (e.g. `org.kde.kirigami as Kirigami`) are also included
/// with `is_qt_module = false` so the caller can treat their types as opaque
/// without reporting `UnknownType`.
pub(super) fn extract_import_aliases(imports: &[String]) -> HashMap<String, bool> {
    let mut aliases = HashMap::new();
    for import_str in imports {
        if let Some(as_pos) = import_str.find(" as ") {
            let before_as = import_str[..as_pos].trim();
            let alias = import_str[as_pos + 4..].trim();
            if !alias.is_empty()
                && alias.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && alias.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                // Determine whether this is a Qt standard module.
                // Module name is the word after "import " (before an optional version number).
                let module_name = before_as
                    .strip_prefix("import ")
                    .unwrap_or(before_as)
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                let is_qt = module_name.starts_with("Qt");
                aliases.insert(alias.to_string(), is_qt);
            }
        }
    }
    aliases
}

// `onWidthChanged` → `widthChanged` lub `width`
pub(super) fn handler_to_signal(handler: &str) -> String {
    let body = &handler[2..]; // strip `on`
    let mut chars = body.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// `widthChanged` → Some("width")
pub(super) fn strip_changed_suffix(s: &str) -> Option<&str> {
    s.strip_suffix("Changed")
}

/// `NewExaminationScreen` → `newExaminationScreen`
pub(super) fn type_name_to_id(type_name: &str) -> String {
    let mut chars = type_name.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

pub(super) fn prop_type_name(t: &PropertyType) -> String {
    match t {
        PropertyType::Int => "int".into(),
        PropertyType::Bool => "bool".into(),
        PropertyType::String => "string".into(),
        PropertyType::Var => "var".into(),
        PropertyType::Double => "double".into(),
        PropertyType::Real => "real".into(),
        PropertyType::Url => "url".into(),
        PropertyType::Color => "color".into(),
        PropertyType::List => "list".into(),
        PropertyType::Custom(s) => s.clone(),
    }
}

pub(super) fn types_compatible(a: &PropertyType, b: &PropertyType) -> bool {
    if matches!(a, PropertyType::Var) || matches!(b, PropertyType::Var) {
        return true;
    }
    // real i double są kompatybilne
    let numeric = |t: &PropertyType| matches!(t, PropertyType::Double | PropertyType::Real);
    if numeric(a) && numeric(b) {
        return true;
    }
    a == b
}

pub(super) fn is_js_global(name: &str) -> bool {
    JS_GLOBALS.contains(&name) || QML_DELEGATE_GLOBALS.contains(&name) || QML_STYLE_GLOBALS.contains(&name)
}

/// Returns `true` when `expr` is an inline QML type instantiation like `Rotation { … }` or
/// `Scale { … }` — an uppercase-starting identifier immediately followed by `{`.
/// Properties inside are not free variable references, so the caller should skip name-checking.
pub(super) fn is_inline_type_instantiation(expr: &str) -> bool {
    let t = expr.trim_start();
    if t.starts_with('{') {
        return true; // pure JS object literal — already handled by callers, kept for convenience
    }
    let mut chars = t.chars();
    let Some(first) = chars.next() else { return false };
    if !first.is_uppercase() {
        return false;
    }
    let rest = chars.as_str();
    let name_end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    rest[name_end..].trim_start().starts_with('{')
}

/// QML-specyficzne zmienne dostępne w kontekście delegatów modeli.
pub(super) const QML_DELEGATE_GLOBALS: &[&str] = &[
    "model",     // obiekt modelu w delegacie (model.field)
    "modelData", // dane elementu w prostym delegacie
    "index",     // indeks elementu w delegacie
];

/// Qt Quick Controls style-attached types available globally when using a style
/// (Material, Universal, Imagine …). No per-file import is required.
pub(super) const QML_STYLE_GLOBALS: &[&str] = &["Material", "Universal", "Imagine"];

pub(super) const JS_GLOBALS: &[&str] = &[
    "console",
    "Math",
    "JSON",
    "parseInt",
    "parseFloat",
    "qsTr",
    "qsTrId",
    "Qt",
    "undefined",
    "null",
    "true",
    "false",
    "NaN",
    "Infinity",
    "String",
    "Number",
    "Boolean",
    "Array",
    "Object",
    "Date",
    "RegExp",
    "Error",
    "Promise",
    "Symbol",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "x",
    "y",
    "z", // często lokalne w lambdach
];
