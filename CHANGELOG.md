# Version 0.2.0 - 01.04.2026
- Fixed false positives: QML ids are now resolved file-wide, Qt namespace qualifiers (e.g. `ListView.Contain`) and inline type instantiations (e.g. `Rotation { … }`) are no longer flagged as undefined
- Improved error locations: errors inside function bodies now point to the exact line instead of the function header
- Extended type-mismatch detection and `Q_PROPERTY MEMBER` backing fields are now recognized in C++ headers
- Added Qt 6.11.0 support for built-in type generation; `check` validates `--path`/`--config` before running
- Added test project, entirelly managed by AI

# Version 0.1.0 - 12.03.2026
- Initial release