# qml_static_analyzer — agent guide

Static analyzer for QML projects written in Rust. Catches common mistakes: typos in property names,
type mismatches, undefined identifiers in JS bodies, invalid signal handlers, and unknown C++
member access.

---

## Source layout

```
src/
  lib.rs                   — re-exports public modules
  main.rs                  — CLI entry point + generate-qt-types + list-builtins subcommands
  cmd_check.rs             — `check` subcommand (BFS usage-path build, per-file error reporting)
  checker/
    mod.rs                 — CheckContext, check_file, Checker impl (scope building + all check_ methods)
    errors.rs              — ErrorKind enum, AnalysisError struct + Display
    helpers.rs             — pure helper functions (handler_to_signal, etc.) + JS_GLOBALS constants
  config.rs                — TOML config parser (Config struct)
  cpp_header.rs            — C++ QObject header parser → extracts Q_PROPERTY / signals / Q_INVOKABLE names
  qt_types.rs              — QtTypeDb: loads from JSON or compiled-in builtin; all_properties / all_signals / all_methods
  qt_types_gen.rs          — reads Qt metatype JSONs from a Qt install → produces qt_types_X.Y.Z.json
  types.rs                 — FileItem, QmlChild, Property, Function, Signal, PropertyType, PropertyValue
  parser/
    mod.rs                 — parse_file entry point; collect_dotted_accesses_from_expression
    core.rs                — recursive descent QML parser
    expression.rs          — JS expression collector (used_names, member_assignments, etc.)
    helpers.rs             — tokeniser helpers
    error.rs               — ParseError type
  snapshot.rs              — snapshot testing helpers (used by integration tests)
build.rs                   — embeds Qt type JSONs listed in INCLUDED_QT_TYPES env var
tests/
  stage1_parser.rs         — parser unit tests (no Qt DB needed)
  stage2_checker.rs        — checker unit tests (no Qt DB needed)
  stage3_checker.rs        — full integration tests (require compiled-in Qt types)
```

---

## Core data flow

```
QML source
  → parser::parse_file()          →  FileItem (properties, children, functions, signals, …)
  → checker::check_file()         →  Vec<AnalysisError>
       uses CheckContext           (known_types, file_members, parent_scopes, cpp_object_members, …)
       uses QtTypeDb               (Qt property/signal/method lookups)
```

`cmd_check` in `cmd_check.rs` orchestrates the full project run:

1. Parse all QML files → `Vec<(PathBuf, FileItem)>`
2. Build `known_types`, `file_members`, `file_base_types`
3. Fixed-point propagate `parent_scopes` (which names each child type inherits from its parent file)
4. BFS from root types (Window / ApplicationWindow / Dialog) → `usage_paths`
5. For each reachable file: `check_file()` → print errors with usage path label

---

## Key design decisions

### `parent` is not validated for member access
`parent` in QML refers to the visual parent whose actual type is only known at runtime (depends on
where the component is instantiated). It is added to scope so it counts as a valid identifier,
but its member access (`parent.foo`) is deliberately not validated — doing so causes false positives
when the actual parent has extra properties (e.g. `parent.radius` on a Rectangle parent,
`parent.goToReportsScreen` on a custom component).

### Type resolution chain
`resolve_qt_type()` walks `file_base_types` until it finds a Qt type:
`TextButton → GenericButton → RoundButton (Qt)`. Used everywhere we need Qt DB lookups.

### parent_scopes — cross-file name visibility
QML's dynamic scoping means names declared in a parent file are accessible in child components.
`parent_scopes[ChildType]` is a `HashSet<String>` of names the child may reference without error.
Built by fixed-point BFS propagation in `cmd_check`.

### Aliased imports
`import QtQuick.Controls as QQC2` → `QQC2.Button` is resolved to `Button` with full Qt validation.
`import org.kde.kirigami as Kirigami` → `Kirigami.Page` is treated as opaque (non-Qt external type).

### C++ objects
Config `[cpp_objects]` declares C++ QObject names. If given a header path, the header is parsed for
`Q_PROPERTY`, `signals:`, `public slots:`, and `Q_INVOKABLE` members. Member access is then
validated. Empty path = opaque (all access allowed). Config `[globals]` lists always-valid names
(macros, test helpers, etc.).

---

## Building and running

```bash
# Build (no Qt types embedded)
cargo build

# Embed specific Qt versions at build time
INCLUDED_QT_TYPES=qt_types_6.8.3.json cargo build

# Generate Qt types JSON from an install
cargo run -- generate-qt-types --qt-path $HOME/Qt/6.8.3/gcc_64
# → qt_types_6.8.3.json

# Run checker (runtime JSON)
cargo run -- check --path gui/src --qt-types-json qt_types_6.8.3.json

# Run checker (compiled-in)
cargo run -- check --path gui/src --builtin-qt-version 6.8.3

# List compiled-in versions
cargo run -- list-builtins
```

### justfile shortcuts

```bash
just gui                        # check with runtime JSON (qt_types_6.8.3.json)
just gui-builtin 6.8.3          # check with compiled-in version
just gen-qt 6.8.3               # generate qt_types_6.8.3.json
just test-with-qt qt_types_6.8.3.json  # run all tests (stage3 needs Qt types compiled in)
just build_with_qt              # build with 6.3.2, 6.8.3, 6.11.0 embedded
```

---

## Qt install paths (this machine)

| Version | Path |
|---------|------|
| 6.3.2   | `~/Qt/6.3.2/gcc_64`  (metatypes in `lib/metatypes/`) |
| 6.8.3   | `~/Qt/6.8.3/gcc_64`  (metatypes in `metatypes/`) |
| 6.11.0  | `~/Qt/6.11.0/gcc_64` (metatypes in `metatypes/`) |

---

## Config file format (TOML)

```toml
[cpp_objects]
diskManager = "relative/path/to/DiskManager.h"   # parsed for members
networkService = ""                               # opaque (all access allowed)

[cpp_singleton]     # legacy alias for cpp_objects (opaque)
[cpp_context]       # legacy alias for cpp_objects (opaque)

[globals]
names = ["AP600_GUI_TEST", "MY_MACRO"]            # always-valid identifiers

[new_child]
MainContent = ["NewExaminationScreen"]            # synthetic child (file-wide)
"MainContent.mainPlain" = ["NewExaminationScreen"]  # child loaded inside a specific loader id

[ignore]
paths = ["vendor/", "generated/"]                 # relative to --path
```

---

## Tests

```bash
# Fast (no Qt types needed)
cargo test --lib
cargo test --test stage1_parser
cargo test --test stage2_checker

# Full (requires Qt types compiled in)
INCLUDED_QT_TYPES=qt_types_6.8.3.json cargo test

# Via justfile
just test-with-qt qt_types_6.8.3.json
```

`stage3_checker.rs` uses `load_default_builtin_db` — it panics if no Qt types are compiled in.

---

## Adding a new error kind

1. Add variant to `ErrorKind` in `src/checker/errors.rs`
2. Add `Display` arm in the same file
3. Emit the error in `src/checker/mod.rs` where appropriate
4. Add a test case in `tests/stage2_checker.rs` or `tests/stage3_checker.rs`
