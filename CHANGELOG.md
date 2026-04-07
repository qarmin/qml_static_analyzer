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