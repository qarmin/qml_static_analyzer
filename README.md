# QML Static Analyzer

This tool is an experimental static analyzer for QML code. It detects type errors, undefined names, incorrect signal handlers, and other common issues without running the application.

It was created because qmllint produced a large number of false positives and was generally difficult to use effectively in our project.

The tool was primarily developed for internal use, where we rely on it to analyze large QML codebases. However, maybe someone will find it useful as well.

It doesn't need to compile project - it works by parsing QML files and analyzing their structure, hierarchy and expressions, so it is really easy to set up and use especially in CI.

## What it detects

| Error                                     | Example                                                         |
|-------------------------------------------|-----------------------------------------------------------------|
| **Property redefinition**                 | `property int height` in a `Rectangle` (already defined by Qt)  |
| **Property type mismatch**                | `property int foo: false` (bool literal, not int)               |
| **Property ref type mismatch**            | `property bool b: otherIntProp` (int property assigned to bool) |
| **Assignment to unknown property**        | `Layout.notExist: true`                                         |
| **Assignment type mismatch**              | `Layout.fillWidth: 21` (expects bool, got int)                  |
| **Undefined name in function**            | `zzzz = x + y` where `zzzz` is not declared anywhere in scope   |
| **Unknown signal handler**                | `onStatussChanged` (typo — no such signal or property)          |
| **Unknown member access**                 | `elem.nonExistingProp = true`                                   |
| **Member assignment type mismatch**       | `elem.boolProp = 0` (int assigned to bool member)               |
| **Undefined name in property expression** | `property bool foo: undeclared.bar`                             |
| **Unknown type**                          | `Sub3 { }` when `Sub3` is not defined anywhere in the project   |
| **Unknown C++ member**                    | `diskManager.nonExisting` when header reveals actual members    |

## Building

```bash
# Build without embedded Qt types (type DB must be provided at runtime)
cargo build --release

# Build with Qt types embedded in the binary
INCLUDED_QT_TYPES=qt_types_6.3.2.json cargo build --release

# Build with multiple Qt versions embedded
INCLUDED_QT_TYPES="qt_types_6.3.2.json,qt_types_6.8.3.json" cargo build --release
```

## Generating the Qt types database

The analyzer needs a JSON file that describes Qt's built-in types. Without it, app would be useless, so this is required. Generate it from an installed Qt:

```bash
# First build the tool
cargo build

# Then generate (point at Qt's gcc_64 directory)
target/debug/qml_static_analyzer generate-qt-types --qt-path ~/Qt/6.3.2/gcc_64
# → produces qt_types_6.3.2.json

# Custom output path or version string
target/debug/qml_static_analyzer generate-qt-types \
    --qt-path ~/Qt/6.8.3/gcc_64 \
    --output my_types.json \
    --qt-version 6.8.3
```

In repo, there are already generated type DB files for Qt 6.3.2 and 6.8.3 that you can use directly or you can generate your own for different versions.

## Usage

### Basic check

```bash
# With type DB embedded in the binary
qml_static_analyzer check --path src/ --builtin-qt-version 6.3.2

# With type DB loaded from file at runtime
qml_static_analyzer check --path src/ --qt-types-json qt_types_6.3.2.json

# With both config file and type DB
qml_static_analyzer check --path src/ --config project.toml --qt-types-json qt_types_6.3.2.json

# With verbose error paths and type DB embedded in the binary
qml_static_analyzer check --path src/ --builtin-qt-version 6.3.2 --complex

# List all built-in Qt types
qml_static_analyzer list-builtins
```

## Config file

Not everything can be detected by static analysis, and some projects may have specific patterns that cause false positives. A config file allows you to ignore specific folders/files, include additional informations about C++ classes and 

## Suppressing individual errors

Add `// qml-ignore` at the end of a line to suppress any error reported on that line:

```qml
invalidProperty2: 123 // qml-ignore
```

By default, a warning is emitted if a `// qml-ignore` comment does not suppress any actual error
(i.e. it is useless). Disable this with `--no-warn-useless-ignore`.

## Known limitations

- **Expressions are not type-checked.** Only simple literals and single-property references are
  checked for type compatibility. `property int x: a + b` is accepted without checking `a` and `b`.
- **`var` properties are unchecked.** Any assignment to a `var`-typed property is accepted.
- **Function return types are not tracked.** Member access on function call results is not validated.
- **JS variable scope is simplified.** `let`/`const`/`var` declarations are collected but not
  strictly scoped within blocks.
- **Enum member existence is not validated.** `Text.AlignLeft` is accepted if `Text` is a known
  Qt type, without checking whether `AlignLeft` actually exists.
- **Loader `source:` files are not checked for existence.** The type name is extracted from the
  path string but the file is not verified.
- **Unused imports are not detected** (planned).
- **C++ header parsing is basic.** Uses pattern matching for `Q_PROPERTY`, signals, and slots.
  Complex macro expansions may not be recognized.

## AI usage
This tool was developed with extensive assistance from AI, guided and directed by humans to accelerate development.