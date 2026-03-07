//! Parser for C++ QObject header files.
//!
//! Extracts QML-accessible member names from:
//! - `Q_PROPERTY(Type Name ...)` declarations
//! - `signals:` section function names
//! - `public slots:` section function names
//! - `Q_INVOKABLE` method names in `public:` sections

use std::collections::HashSet;

/// Parse a C++ QObject header and return all QML-accessible member names.
pub fn parse_cpp_header(source: &str) -> HashSet<String> {
    let mut members = HashSet::new();
    let cleaned = strip_comments(source);
    extract_q_properties(&cleaned, &mut members);
    extract_signals_and_slots(&cleaned, &mut members);
    members
}

// ── comment stripping ─────────────────────────────────────────────────────────

fn strip_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    result.push('\n');
                }
                i += 1;
            }
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

// ── Q_PROPERTY extraction ─────────────────────────────────────────────────────

fn extract_q_properties(source: &str, members: &mut HashSet<String>) {
    let marker = "Q_PROPERTY(";
    let mut pos = 0;
    while pos < source.len() {
        let Some(rel) = source[pos..].find(marker) else { break };
        let start = pos + rel + marker.len();
        let content_end = find_matching_paren(source, start);
        let content = &source[start..content_end];
        if let Some(name) = extract_property_name(content) {
            members.insert(name);
        }
        pos = content_end + 1;
    }
}

/// Find the position of the closing `)` that matches the opening `(` at `start`
/// (the `(` itself was already consumed — `start` is right after it).
fn find_matching_paren(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    source.len()
}

/// Keywords that terminate the "type + name" prefix inside Q_PROPERTY(…).
const Q_PROPERTY_KEYWORDS: &[&str] = &[
    "MEMBER",
    "READ",
    "WRITE",
    "NOTIFY",
    "RESET",
    "CONSTANT",
    "FINAL",
    "REQUIRED",
    "REVISION",
    "DESIGNABLE",
    "SCRIPTABLE",
    "STORED",
];

/// Extract the property name from the content of `Q_PROPERTY(…)`.
///
/// Format: `TypeExpr PropertyName KEYWORD …`
/// The property name is the last identifier that appears immediately before the
/// first Q_PROPERTY keyword.
fn extract_property_name(content: &str) -> Option<String> {
    let tokens = tokenize_q_property_content(content);
    let keyword_pos = tokens.iter().position(|t| Q_PROPERTY_KEYWORDS.contains(&t.as_str()))?;
    if keyword_pos == 0 {
        return None;
    }
    let name = &tokens[keyword_pos - 1];
    if is_plain_identifier(name) && !is_cpp_type_keyword(name) {
        Some(name.clone())
    } else {
        None
    }
}

/// Tokenize Q_PROPERTY content, keeping template arguments (`<…>`) as a single token.
fn tokenize_q_property_content(content: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    let mut angle_depth = 0usize;

    #[allow(clippy::allow_attributes)]
    #[allow(clippy::match_same_arms)]
    while i < chars.len() {
        let c = chars[i];
        match c {
            '<' => {
                angle_depth += 1;
                current.push(c);
            }
            '>' => {
                angle_depth = angle_depth.saturating_sub(1);
                current.push(c);
            }
            ' ' | '\t' | '\n' | '\r' if angle_depth == 0 => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            '*' | '&' if angle_depth == 0 => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                // Discard pointer/reference symbols — they are part of the type, not the name
            }
            _ => {
                current.push(c);
            }
        }
        i += 1;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_plain_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':')
}

fn is_cpp_type_keyword(s: &str) -> bool {
    matches!(
        s,
        "const"
            | "volatile"
            | "unsigned"
            | "signed"
            | "long"
            | "short"
            | "int"
            | "bool"
            | "char"
            | "void"
            | "float"
            | "double"
    )
}

// ── signals / slots extraction ────────────────────────────────────────────────

#[derive(PartialEq)]
enum Section {
    Other,
    Signals,
    PublicSlots,
    Public, // for Q_INVOKABLE
}

fn extract_signals_and_slots(source: &str, members: &mut HashSet<String>) {
    let mut section = Section::Other;

    for raw_line in source.lines() {
        let line = raw_line.trim();

        // Section markers — check these before anything else
        if matches_section_marker(line, &["signals:", "Q_SIGNALS:"]) {
            section = Section::Signals;
            continue;
        }
        if matches_section_marker(line, &["public slots:", "public Q_SLOTS:", "Q_SLOTS:"]) {
            section = Section::PublicSlots;
            continue;
        }
        if line == "public:" {
            section = Section::Public;
            continue;
        }
        // Other access specifiers end the tracked sections
        if matches_section_marker(
            line,
            &[
                "protected:",
                "private:",
                "private slots:",
                "private Q_SLOTS:",
                "protected slots:",
                "protected Q_SLOTS:",
            ],
        ) {
            section = Section::Other;
            continue;
        }

        match section {
            Section::Signals | Section::PublicSlots => {
                if let Some(name) = extract_function_name(line) {
                    members.insert(name);
                }
            }
            Section::Public => {
                if let Some(rest) = line.strip_prefix("Q_INVOKABLE")
                    && let Some(name) = extract_function_name(rest)
                {
                    members.insert(name);
                }
            }
            Section::Other => {}
        }
    }
}

fn matches_section_marker(line: &str, markers: &[&str]) -> bool {
    markers.contains(&line)
}

/// Extract the function name from a C++ method declaration line.
/// E.g. `void startPeriodicalScan(int interval)` → `startPeriodicalScan`
///      `bool unmountSelectedDisk()` → `unmountSelectedDisk`
fn extract_function_name(line: &str) -> Option<String> {
    let line = line.trim_end_matches(';').trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return None;
    }
    // Find first `(` — the function name is the last word before it
    let paren_pos = line.find('(')?;
    let before = &line[..paren_pos];
    let last_word = before.split_whitespace().last()?;
    // Strip any trailing type qualifiers or template args that got attached
    let name = last_word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
    if is_plain_identifier(name) && !is_cpp_type_keyword(name) {
        Some(name.to_string())
    } else {
        None
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_diskmanager_header() {
        let src = "
class DiskManager : public QObject {
    Q_OBJECT
signals:
    void diskListChanged();
    void selectedDiskIndexChanged();
public:
    Q_PROPERTY(QList<QObject *> portableDisks MEMBER m_diskList NOTIFY diskListChanged)
    Q_PROPERTY(int selectedDiskIndex MEMBER m_selectedDiskIndex NOTIFY selectedDiskIndexChanged)
    Q_PROPERTY(bool isProcessingDevices READ isProcessingDevices NOTIFY processingDevicesListChanged)
public slots:
    void startPeriodicalScan(int interval);
    bool unmountSelectedDisk();
};
";
        let members = parse_cpp_header(src);
        assert!(members.contains("portableDisks"), "{members:?}");
        assert!(members.contains("selectedDiskIndex"), "{members:?}");
        assert!(members.contains("isProcessingDevices"), "{members:?}");
        assert!(members.contains("diskListChanged"), "{members:?}");
        assert!(members.contains("selectedDiskIndexChanged"), "{members:?}");
        assert!(members.contains("startPeriodicalScan"), "{members:?}");
        assert!(members.contains("unmountSelectedDisk"), "{members:?}");
    }

    #[test]
    fn strips_line_comments() {
        let src = "// comment\nQ_PROPERTY(int foo MEMBER m_foo NOTIFY fooChanged)\n";
        let members = parse_cpp_header(src);
        assert!(members.contains("foo"), "{members:?}");
    }

    #[test]
    fn strips_block_comments() {
        let src = "/* block */ Q_PROPERTY(int bar MEMBER m_bar NOTIFY barChanged)";
        let members = parse_cpp_header(src);
        assert!(members.contains("bar"), "{members:?}");
    }
}
