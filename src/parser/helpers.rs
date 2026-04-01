//! Small, stateless parsing helpers: comment stripping, type-open detection,
//! signal/property/function-header parsing, and loader-source extraction.

use super::expression::collect_base_names_from_expression;
use crate::types::{Property, PropertyType, PropertyValue, Signal, SignalParameter};

// ─── comment stripping ──────────────────────────────────────────────────────

/// Remove `//` line comments (but leave string content intact for simplicity).
pub fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut prev = ' ';
    for (i, ch) in line.char_indices() {
        match ch {
            '"' | '\'' if prev != '\\' => in_string = !in_string,
            '/' if !in_string && prev == '/' => {
                return &line[..i - 1];
            }
            _ => {}
        }
        prev = ch;
    }
    line
}

/// Remove `/* … */` block comments from source, preserving newlines so that
/// line numbers remain accurate.
pub fn strip_block_comments(source: &str) -> String {
    if !source.contains("/*") {
        return source.to_string();
    }
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next(); // consume '/'
                        break;
                    }
                    Some('\n') => result.push('\n'), // preserve line numbers
                    None => break,
                    _ => {}
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ─── type-open detection ────────────────────────────────────────────────────

/// Try to parse a line like `TypeName {` → Some("TypeName")
/// Also handles `TypeName.SubType {` (e.g. `QtObject {`)
pub fn parse_type_open(line: &str) -> Option<String> {
    let line = strip_comment(line).trim();
    if !line.ends_with('{') {
        return None;
    }
    let without_brace = line[..line.len() - 1].trim();
    if without_brace.is_empty() {
        return None;
    }
    // Must NOT contain ':' (that would be an assignment)
    if without_brace.contains(':') {
        return None;
    }
    // Handle "Behavior on <property>" — strip the " on <prop>" qualifier
    let type_name = if let Some(pos) = without_brace.find(" on ") {
        without_brace[..pos].trim()
    } else {
        without_brace
    };
    // Must start with uppercase letter (QML types start with uppercase)
    if type_name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return Some(type_name.to_string());
    }
    None
}

/// Parse single-line element: `TypeName { }` or `TypeName {}`.
/// Also handles `Behavior on prop { }`. Returns the type name if matched.
pub fn parse_type_open_single_line(line: &str) -> Option<String> {
    let line = strip_comment(line).trim();
    if !line.ends_with('}') {
        return None;
    }
    let brace_pos = line.find('{')?;
    let type_part = line[..brace_pos].trim();
    if type_part.is_empty() || type_part.contains(':') {
        return None;
    }
    // Handle "Behavior on <property>" — strip the " on <prop>" qualifier
    let type_name = if let Some(pos) = type_part.find(" on ") {
        type_part[..pos].trim()
    } else {
        type_part
    };
    if type_name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return Some(type_name.to_string());
    }
    None
}

// ─── id parsing ─────────────────────────────────────────────────────────────

/// Try to parse `id: someId`
pub fn try_parse_id(line: &str) -> Option<String> {
    let line = strip_comment(line).trim();
    let rest = line.strip_prefix("id:")?;
    let val = rest.trim().trim_end_matches(';').to_string();
    if val.is_empty() { None } else { Some(val) }
}

// ─── loader source extraction ───────────────────────────────────────────────

/// For a `source: "qrc:/some/path/TypeName.qml"` line (or a ternary that contains
/// one or more `.qml"` literals) return every QML stem found as a child type name.
///
/// Examples:
/// * `source: "qrc:/foo/WelcomeContent.qml"`  → `["WelcomeContent"]`
/// * `source: flag ? "qrc:/a/A.qml" : "qrc:/b/B.qml"` → `["A", "B"]`
/// * `source: flag ? "qrc:/a/A.qml" : ""`    → `["A"]`
pub fn extract_loader_source_types(line: &str) -> Vec<String> {
    let line = strip_comment(line).trim();
    // Must start with "source:"
    let rest = if let Some(after_source) = line.strip_prefix("source") {
        let after = after_source.trim_start();
        if let Some(after_colon) = after.strip_prefix(':') {
            after_colon.trim()
        } else {
            return vec![];
        }
    } else {
        return vec![];
    };
    // Collect all quoted strings ending in .qml
    let mut results = Vec::new();
    let mut remaining = rest;
    while let Some(dot_pos) = remaining.find(".qml") {
        // walk back to find the start of the filename (after last / " or ')
        let before = &remaining[..dot_pos];
        let stem_start = before.rfind(['/', '"', '\'']).map_or(0, |p| p + 1);
        let stem = &before[stem_start..];
        if !stem.is_empty() && stem.chars().next().is_some_and(|c| c.is_uppercase()) {
            results.push(stem.to_string());
        }
        remaining = &remaining[dot_pos + 4..]; // skip past ".qml"
    }
    results
}

// ─── signal parsing ─────────────────────────────────────────────────────────

/// Parse `signal name()` or `signal name(type param, type2 param2)`
pub fn parse_signal_decl(line: &str) -> Option<Signal> {
    let line = strip_comment(line).trim();
    let rest = line.strip_prefix("signal ")?.trim();
    // Find optional parameter list
    let (name, params_str) = if let Some(paren_pos) = rest.find('(') {
        let n = rest[..paren_pos].trim().to_string();
        let close = rest.find(')')?;
        let p = &rest[paren_pos + 1..close];
        (n, Some(p))
    } else {
        (rest.trim_end_matches(';').to_string(), None)
    };

    if name.is_empty() {
        return None;
    }

    let parameters = params_str.map(parse_signal_params).unwrap_or_default();

    Some(Signal { name, parameters })
}

fn parse_signal_params(params: &str) -> Vec<SignalParameter> {
    if params.trim().is_empty() {
        return vec![];
    }
    params
        .split(',')
        .filter_map(|p| {
            let parts: Vec<&str> = p.trim().splitn(2, char::is_whitespace).collect();
            if parts.len() == 2 {
                Some(SignalParameter {
                    param_type: parts[0].to_string(),
                    param_name: parts[1].to_string(),
                })
            } else if parts.len() == 1 && !parts[0].is_empty() {
                Some(SignalParameter {
                    param_type: parts[0].to_string(),
                    param_name: String::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

// ─── property parsing ───────────────────────────────────────────────────────

/// Parse `property <type> <name>` or `property <type> <name>: <value>`
pub fn parse_property_decl(line: &str) -> Option<Property> {
    let line = strip_comment(line).trim();
    let rest = line.strip_prefix("property ")?.trim();

    // Split on whitespace to get type and rest
    let mut parts = rest.splitn(2, char::is_whitespace);
    let type_str = parts.next()?.trim();
    let after_type = parts.next()?.trim();

    // after_type is `name` or `name: value`
    let (name, value_str) = if let Some(colon_pos) = after_type.find(':') {
        let n = after_type[..colon_pos].trim().to_string();
        let v = after_type[colon_pos + 1..].trim().to_string();
        (n, Some(v))
    } else {
        (after_type.trim_end_matches(';').to_string(), None)
    };

    if name.is_empty() {
        return None;
    }

    let prop_type = PropertyType::from_token(type_str);

    let (value, accessed_properties, is_simple_ref) = match value_str.as_deref() {
        None | Some("") => (PropertyValue::Unset, vec![], false),
        Some(v) => parse_property_value(v),
    };

    Some(Property {
        name,
        prop_type,
        value,
        accessed_properties,
        is_simple_ref,
        line: 0, // will be filled in by the caller
    })
}

/// Attempt to parse a property value expression.
/// Returns (PropertyValue, accessed_properties, is_simple_ref).
/// `is_simple_ref` is true when the expression is exactly one plain identifier
/// (e.g. `other`), allowing cross-property type checking.
pub fn parse_property_value(expr: &str) -> (PropertyValue, Vec<String>, bool) {
    let expr = expr.trim().trim_end_matches(';');

    // Boolean literals
    if expr == "true" {
        return (PropertyValue::Bool(true), vec![], false);
    }
    if expr == "false" {
        return (PropertyValue::Bool(false), vec![], false);
    }
    // Null
    if expr == "null" || expr == "undefined" {
        return (PropertyValue::Null, vec![], false);
    }
    // Integer literal
    if let Ok(i) = expr.parse::<i64>() {
        return (PropertyValue::Int(i), vec![], false);
    }
    // Float literal
    if let Ok(f) = expr.parse::<f64>() {
        return (PropertyValue::Double(f), vec![], false);
    }
    // String literal
    if (expr.starts_with('"') && expr.ends_with('"')) || (expr.starts_with('\'') && expr.ends_with('\'')) {
        let inner = expr[1..expr.len() - 1].to_string();
        return (PropertyValue::String(inner), vec![], false);
    }

    // Complex expression – collect accessed names
    let accessed = collect_base_names_from_expression(expr);
    // Simple ref: expression is exactly one plain identifier, e.g. `property bool b: other`
    let is_simple_ref = accessed.len() == 1 && accessed[0].as_str() == expr;
    (PropertyValue::TooComplex, accessed, is_simple_ref)
}

// ─── function header parsing ────────────────────────────────────────────────

/// Parse `function name(params) {` header.
/// Returns (name, params, has_opening_brace).
pub fn parse_function_header(line: &str) -> Option<(String, Vec<String>, bool)> {
    let line = strip_comment(line).trim();
    let rest = line.strip_prefix("function ")?.trim();

    let paren_pos = rest.find('(')?;
    let name = rest[..paren_pos].trim().to_string();

    let close_paren = rest.find(')')?;
    let params_str = &rest[paren_pos + 1..close_paren];
    let params: Vec<String> = params_str
        .split(',')
        .map(|p| {
            let s = p.trim();
            // Strip default value: `param = default` → `param`
            let s = s.find('=').map_or(s, |eq| s[..eq].trim());
            // Strip QML 6 type annotation: `param: Type` → `param`
            // e.g. `x: var`, `text: string`, `menu: PlasmaExtras.Menu`
            s.find(':').map_or(s, |colon| s[..colon].trim()).to_string()
        })
        .filter(|p| !p.is_empty())
        .collect();

    let after_paren = rest[close_paren + 1..].trim();
    let has_open_brace = after_paren.contains('{');

    Some((name, params, has_open_brace))
}

// ─── property-as-element detection ──────────────────────────────────────────

/// Try to parse a property assignment whose RHS is an inline multi-line element:
///   `propName: TypeName {`
/// Returns the type name if matched.  Signal handlers (`onXxx:`) and dotted
/// keys (`Layout.xxx:`) are excluded — they are handled elsewhere.
pub fn try_parse_property_element_open(line: &str) -> Option<String> {
    let line = strip_comment(line).trim();
    if !line.ends_with('{') {
        return None;
    }
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim();
    // Key must be a plain identifier: no spaces, no dots
    if key.is_empty() || key.contains(' ') || key.contains('.') {
        return None;
    }
    // RHS (between `:` and trailing `{`) must be an uppercase-starting type name
    let rhs = line[colon_pos + 1..].trim(); // e.g. "Rectangle {"
    let type_name = rhs.trim_end_matches('{').trim(); // e.g. "Rectangle"
    if type_name.is_empty() {
        return None;
    }
    if type_name.chars().next().is_some_and(|c| c.is_uppercase()) {
        Some(type_name.to_string())
    } else {
        None
    }
}

// ─── signal handler detection ───────────────────────────────────────────────

/// Returns true if the line looks like `onSomeHandler: {` or `onSomeHandler: expr`
pub fn is_signal_handler_block(line: &str) -> bool {
    let line = strip_comment(line).trim();
    if let Some(colon_pos) = line.find(':') {
        let key = line[..colon_pos].trim();
        if key.contains(' ') {
            return false; // property declaration like "property bool foo"
        }
        // Direct form: "onXxx"
        if key.starts_with("on") && key.len() > 2 && key.chars().nth(2).is_some_and(|c| c.is_uppercase()) {
            return true;
        }
        // Attached type form: "TypeName.onXxx"  e.g. "Component.onCompleted"
        if let Some(dot_pos) = key.rfind('.') {
            let after_dot = &key[dot_pos + 1..];
            if after_dot.starts_with("on")
                && after_dot.len() > 2
                && after_dot.chars().nth(2).is_some_and(|c| c.is_uppercase())
            {
                return true;
            }
        }
    }
    false
}
