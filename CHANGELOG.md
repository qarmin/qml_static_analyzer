# Unreleased
- Added `[roots] treat_unreachable_as_roots` config flag: any file BFS still can't reach after the normal root pass becomes its own root and gets checked anyway. For frameworks (e.g. KDE Plasma's KPackage) that load QML by path convention rather than a small set of known root element types, enumerating every possible root type name doesn't scale — this sidesteps type-name matching entirely. Defaults to `false`.
- Fixed: `check_root` didn't respect `[external_types]` for a file's own (unaliased) base type — only `check_child` did, so a file rooted at a configured external type still had its own top-level property assignments flagged as `UnknownPropertyAssignment`
- Fixed (pre-existing since aliased-import support): signal handlers were validated unconditionally, ignoring the opaque/partial-validation state of aliased or external root/child types — every handler on an opaque element was a guaranteed false positive. Now skipped when the element's resolved property/signal set is empty.
- `suggest-config` now also considers a file's own root base type (not just child element types) when suggesting `[external_types]` candidates
- Added `[external_types]` config section: type names from compiled C++/QML modules the analyzer cannot introspect (e.g. Quickshell's `ShellRoot`/`PanelWindow`/`Scope`, KDE's `PlasmoidItem`) no longer raise `UnknownType` and skip own-property validation, while their children are still checked normally
- Added `[roots]` config section (`extra_types`): extra BFS reachability root element types on top of the built-in `Window`/`ApplicationWindow`/`Dialog` — needed for non-Qt-Quick entry points like Quickshell's `ShellRoot`/`Scope`
- Added `extends` config field: a config can now inherit from a base config/preset (list-valued sections union, map-valued sections merge with the more specific config winning); cycles are detected and ignored with a warning
- Added `suggest-config` subcommand: scans a project without compiling it and prints a starter `[external_types]`/`[roots]` snippet, for quickly bootstrapping a config on a new project
- `generate-qt-types` gained a repeatable `--extra-metatypes-dir <dir>` flag: recursively scans a build tree for any `*_metatypes.json` (the standard moc/AUTOMOC format, not specific to Qt itself) and merges it into the generated type DB, so third-party C++/QML modules get real property/signal/method validation instead of being treated as opaque — when the target project has actually been built
- Added `presets/quickshell.toml`, a best-effort starter preset for Quickshell-based shell projects, usable via `extends`
- See `docs/cpp_projects_extension_ideas.md` for the full list of investigated extension points towards C++-heavy / non-Qt-Quick-native projects (Quickshell shells, KDE Plasma)

# Version 0.4.1 - 13.04.2026
- Errors for `UnknownMemberAccess` and `MemberAssignmentTypeMismatch` now point to the exact assignment line instead of the function header
- Improved parser: multi-line expressions (open parentheses spanning multiple lines) now correctly collect `used_names`, `declared_locals`, and `member_assignments` from every continuation line
- RHS names in member assignments (e.g. `obj.x = bad + 1`) are now checked for undefined identifiers
- Arrow-function parameters support destructuring: `({prop})`, `([a, b])` patterns are correctly extracted as local bindings
- `for`-loop object-destructuring (`for (const {key: alias} of ...)`) is now supported
- `try_parse_member_assignment` correctly marks multi-token RHS as `TooComplex` instead of misclassifying them
- `MemberAssignment` stores the source line number for accurate error reporting; `PartialEq` intentionally ignores it so snapshots remain stable

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