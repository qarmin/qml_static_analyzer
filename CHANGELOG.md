# Version 0.4.0 - 10.04.2026
- Added `UnknownQmlMember` error: member access on known QML child ids (e.g. `childId.nonExistentProp`) is now validated in function bodies, property declarations, and inline assignments
- Added `UnknownConnectionsTarget` error: `Connections { target: unknownThing }` is now caught when the target is not in scope
- Improved `Connections` validation: signal handler names are checked against the actual target's signals/properties; opaque or unresolved targets allow all declared handlers
- Added cross-file id member access validation via `parent_id_types`: element ids declared in parent files are now visible for member-access checks in child files
- Auto-generated `<prop>Changed` signals are now recognised as valid identifiers in all contexts
- `parent` now resolves to the actual parent element type for direct children, enabling `parent.xxx` validation without false positives
- JS-block property value bodies (`text: { if (foo) … }`) are now checked for undefined names
- Qt type methods and signals are added to file scope, preventing false `UndefinedName` errors for callable Qt members
- `file_members` now includes function names from QML files, improving cross-file scope resolution
- Refactored source: `checker.rs` split into `checker/mod.rs`, `checker/errors.rs`, `checker/helpers.rs`; `check` subcommand moved to `cmd_check.rs`
- CLI: added `--version` / `-V` and `--help` / `-h` flags; version printed on every `check` run
- Release profile hardened: `lto = "fat"`, `codegen-units = 1`, `overflow-checks = true`

# Version 0.3.0 - 07.04.2026
- Added support for aliased module imports (e.g. `import QtQuick.Controls as QQC2`) — aliased types are now resolved correctly without false `UnknownType` errors
- Improved parser: handles `async function`, dotted property assignments (`icon.source`), multi-line signal handlers, and `},`/`};` as valid block closers
- Extended C++ member validation to cover property declaration expressions, not only function bodies

# Version 0.2.0 - 01.04.2026
- Fixed false positives: QML ids are now resolved file-wide, Qt namespace qualifiers (e.g. `ListView.Contain`) and inline type instantiations (e.g. `Rotation { … }`) are no longer flagged as undefined
- Improved error locations: errors inside function bodies now point to the exact line instead of the function header
- Extended type-mismatch detection and `Q_PROPERTY MEMBER` backing fields are now recognized in C++ headers
- Added Qt 6.11.0 support for built-in type generation; `check` validates `--path`/`--config` before running
- Added test project, entirelly managed by AI

# Version 0.1.0 - 12.03.2026
- Initial release