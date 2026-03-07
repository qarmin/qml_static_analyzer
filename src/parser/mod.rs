//! QML parser – converts raw `.qml` text into [`FileItem`].
//!
//! # Module layout
//!
//! | sub-module     | contents                                              |
//! |----------------|-------------------------------------------------------|
//! | [`error`]      | [`ParseError`] type                                   |
//! | [`helpers`]    | Stateless line-level parsing helpers                  |
//! | [`expression`] | JS expression tokeniser and name-collection utilities |
//! | [`core`]       | `Parser` state machine                                |
mod core;
pub mod error;
pub mod expression;
pub mod helpers;
use core::Parser;

pub use error::ParseError;
pub use expression::{
    collect_base_names_from_expression, collect_dotted_accesses_from_expression, collect_names_from_expression,
};
use helpers::strip_block_comments;

use crate::types::FileItem;
/// Parse the contents of a single `.qml` file.
pub fn parse_file(name: &str, source: &str) -> Result<FileItem, ParseError> {
    let processed = strip_block_comments(source);
    let mut parser = Parser::new(&processed);
    parser.parse_file(name)
}
/// Parse all `.qml` files found directly inside `dir`.
pub fn parse_directory(dir: &std::path::Path) -> Result<Vec<FileItem>, ParseError> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| ParseError {
            line: 0,
            message: format!("Cannot read dir {dir:?}: {e}"),
        })?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("qml"))
        .collect();
    entries.sort();
    let mut result = Vec::with_capacity(entries.len());
    for path in &entries {
        let source = std::fs::read_to_string(path).map_err(|e| ParseError {
            line: 0,
            message: format!("Cannot read {path:?}: {e}"),
        })?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
        let item = parse_file(name, &source)?;
        result.push(item);
    }
    Ok(result)
}
