# Handoff — extending qml_static_analyzer to non-plain-QML projects

Working notes to resume this work in a fresh session. Everything described as "done" below
is already committed on branch `feature/external-projects-support` (5 commits, not merged
to `master`, working tree clean as of writing this). `master` is untouched.

```
git log --oneline master..feature/external-projects-support
fceb1bf Document treat_unreachable_as_roots and the plasma-bigscreen findings
4969ec6 Add treat_unreachable_as_roots; fix opaque-type gaps found on plasma-bigscreen
0f3c754 Document extends/suggest-config/metatypes-scan workflow
fda38a0 Add config extends, suggest-config, and third-party metatypes scanning
45bcec4 Add [external_types] and [roots] config for external QML frameworks
```

## Context / origin

User wanted to check whether this QML static analyzer (built for a plain-Qt-Quick project
with no custom C++ types) could be extended to work on other, more complex real-world
projects that were cloned into `temp/` (gitignored, scratch area): `Ambxst`,
`DankMaterialShell`, `WhiteSur-kde`, `cool-retro-term`, `iDescriptor`, `iNiR`,
`plasma-bigscreen`, `plasma-desktop`, `shell`. Two families: Quickshell-based shells
(compiled C++ framework, 0 local C++ files, everything via `import Quickshell.*`) and KDE
Plasma projects (real local C++, KDE Frameworks/Plasma modules via aliased imports).

Full survey + prioritized Tier 1/2/3 list of extension ideas lives in
`docs/cpp_projects_extension_ideas.md` — READ THAT FIRST, it's the source of truth for
what's done vs still open, with checkboxes.

**Constraint from the user, repeatedly emphasized**: their own project is plain QML, no C++
types — every change must be purely additive/opt-in via config, defaulting to old behavior,
verified by `just test` (98 tests) + `cargo clippy --all-targets` showing zero regressions.

## What's implemented and working (all opt-in via config, defaults unchanged)

1. **`[external_types]`** (config.rs, checker/mod.rs) — unqualified element type names from
   compiled C++/QML modules the analyzer can't introspect (e.g. Quickshell's `ShellRoot`,
   `PanelWindow`; KDE's `ActionButton`). No `UnknownType`, no own-property/signal validation,
   but **children are still checked normally** (unlike the older aliased-import opaque path
   for e.g. `Kirigami.Icon`, which skips the whole subtree).
2. **`[roots] extra_types`** — extra BFS reachability root types besides the hardcoded
   `Window`/`ApplicationWindow`/`Dialog` (`ROOT_QT_TYPES` in cmd_check.rs).
3. **`[roots] treat_unreachable_as_roots`** (bool) — any file BFS still can't reach becomes
   its own root anyway. Added specifically because KDE Plasma's KPackage loads QML by path
   convention, not a small enumerable set of root types — `suggest-config` found 38 distinct
   base types on plasma-bigscreen alone, unmaintainable as a list.
4. **`extends`** config field (config.rs: `load_config_file`, `merge_configs`) — a config can
   inherit from a base config/preset, resolved relative to the declaring file's own
   directory. List sections union; map sections merge key-by-key, child wins on conflict.
   Cycle-safe. `presets/quickshell.toml` is a worked example.
5. **`suggest-config` subcommand** (cmd_check.rs `cmd_suggest_config`, wired in main.rs) —
   runs the same parse+BFS pipeline as `check` without compiling anything, prints candidate
   `[external_types]`/`[roots]` entries instead of errors. Heuristic, meant for human review.
   Scans BOTH child element types AND each file's own root base type.
6. **`generate-qt-types --extra-metatypes-dir <dir>`** (repeatable; qt_types_gen.rs
   `generate_module_types_json` + `find_all_metatypes_files`/`load_extra_classes`) —
   recursively scans a build tree for any `*_metatypes.json` (standard moc/AUTOMOC format,
   not Qt-specific) and merges it into the generated type DB, so a third-party module's real
   C++ types get real property/signal/method validation instead of opaque treatment — when
   the target project has actually been built. Qt's own classes win on name clashes (needed
   as inheritance root).
7. **Two real bugs found and fixed** while dogfooding on plasma-bigscreen:
   - `check_root` didn't consult `[external_types]` for a file's own base type (only
     `check_child` did) — a file rooted at an external type had its own properties flagged.
   - `check_function`'s signal-handler validation ran unconditionally, ignoring opacity
     entirely (bug pre-dated this session, since aliased-import support in 0.3.0) — every
     handler on an opaque root/child was a guaranteed false positive. Fixed by skipping when
     `qt_props`/`qt_signals`/`declared_signals` are all empty (reliable proxy for "opaque",
     since every resolvable Qt type has at least `objectName` from `QtObject`).

Real-world verification: `temp/plasma-bigscreen` (KDE Plasma, 106 QML + 88 C++ files) went
from **99 errors / 93 of 106 files unchecked** (baseline, no config) to **290 errors / 0
unchecked** (with config + the two fixes) — more errors because far more of the project is
now actually checked, not a regression. Full narrative in
`temp/plasma-bigscreen/ANALYZER_NOTES.md` + `temp/plasma-bigscreen/analyzer_config.toml`
(both gitignored, local only — not lost, just not in git history; re-derivable from this
doc + `docs/cpp_projects_extension_ideas.md` if needed, or just re-run `suggest-config`).

Tests: `tests/stage3_checker.rs` has new regression tests for both bug fixes plus the
`[external_types]`-as-root-base-type case. `src/config.rs` and `src/qt_types_gen.rs` have
`#[cfg(test)]` unit tests for `extends`/cycle-detection and the metatypes merge
(synthetic fixtures — no real Qt SDK was available on this machine to test against).

Known pre-existing failure, NOT caused by any of this work (confirmed via `git stash` +
diff against clean `master` before starting): `test_parent_invalid_member_flagged` in
`tests/stage3_checker.rs`. Ignore it — it was already failing on `master`.

## What was IN PROGRESS when this session ran out of budget

Working on the last open item from `docs/cpp_projects_extension_ideas.md`'s "trzecia runda"
section: **`qt_types_gen.rs`'s hardcoded `MODULES` list (in `load_all_classes`,
around line 107) is missing several real Qt6 modules**, so their types have to go in
`[external_types]` (opaque) on plasma-bigscreen instead of getting real validation:
`QtQuick.Effects` (`MultiEffect`, `DropShadow`), `QtQuick.Shapes` (`Shape`, `ShapePath`),
`QtMultimedia` (`Camera`, `MediaDevices`, `CaptureSession`, `VideoOutput`, `SoundEffect`),
`QtWebEngine` (`WebEngineView`), possibly `Qt.labs.*` modules.

Confirmed via checking the 3 bundled `qt_types_*.json` files: `MultiEffect`/`DropShadow`/
`Shape`/`FolderListModel` are absent from all three (6.3.2, 6.8.3, 6.11.0). `DelegateChooser`
IS present in 6.11.0 but not 6.3.2/6.8.3 — that one's just a real Qt version difference
(the type didn't exist/wasn't QML_ELEMENT-registered that early), NOT a module-list gap —
don't "fix" that one.

**Two possible approaches, not yet decided/implemented:**

1. **Widen the hardcoded `MODULES` list** in `load_all_classes` (qt_types_gen.rs:107) with
   best-effort additional entries (`qt6quickeffects`, `qt6quickshapes` or
   `qt6quickshapesprivate`, `qt6multimedia`, `qt6webenginequick` or `qt6webengine`,
   `qt6labsfolderlistmodel`, `qt6labsplatform`, `qt6labssettings`, `qt6quickparticles`).
   Additive and safe (missing files are silently skipped, per existing comment at line
   104), but **exact CMake target → metatypes filename mapping could not be verified on
   this machine — there is no Qt SDK installed here** (`~/Qt/` doesn't exist). Don't trust
   the exact module-id strings without checking a real install's `metatypes/` dir listing
   first (`ls ~/Qt/6.8.3/gcc_64/metatypes/ | grep -i effect` etc.), or without a machine
   that has Qt installed to actually regenerate and diff.

2. **(Recommended — realized mid-investigation, not yet acted on) Just document the
   existing `--extra-metatypes-dir` flag as the fix, no new code needed**: since
   `generate_module_types_json` already does a fully generic recursive `*_metatypes.json`
   scan (`find_all_metatypes_files`) merged on top of the curated Qt classes, pointing it at
   the Qt install's OWN metatypes directory works as a catch-all for anything the curated
   list misses:
   ```bash
   qml_static_analyzer generate-qt-types --qt-path ~/Qt/6.8.3/gcc_64 \
       --extra-metatypes-dir ~/Qt/6.8.3/gcc_64/metatypes \
       --output qt_types_6.8.3_full.json
   ```
   This needs zero code changes — just verify it actually works end-to-end (it should,
   by construction: `load_extra_classes` recursively finds every `*_metatypes.json`
   regardless of name) and add it to the README/docs as the answer to "a Qt module is
   missing from `list-builtins`". **This was not yet tested or written down** — do that
   first, it's cheaper and safer than guessing module-id strings for option 1.

Recommendation for next session: do **option 2 first** (test it, document it, zero risk),
then decide if option 1 (widening the default list for convenience, so people don't need
the extra flag for common modules) is still worth doing on top.

## How to resume

1. `git checkout feature/external-projects-support` (or it's already checked out).
2. Read `docs/cpp_projects_extension_ideas.md` top to bottom — it's the authoritative
   tracking doc, checkboxes show done vs open.
3. For the in-progress item above: try option 2 first. Since there's no Qt SDK on this
   machine, testing "does `--extra-metatypes-dir` pointed at a real Qt metatypes dir pick up
   MultiEffect/DropShadow/etc." requires either (a) a machine with Qt installed, or (b) a
   synthetic fixture the way `qt_types_gen.rs`'s existing test
   (`extra_metatypes_dir_merges_third_party_module_with_qt_inheritance`) does — fake a
   `MultiEffect`-like class JSON and confirm it merges in.
4. Standard verification loop for ANY change here: `cargo build` →
   `just test` (expect 99 passed, 1 pre-existing fail) → `cargo clippy --all-targets` (diff
   against `git stash`-ed clean state if unsure whether a warning is new) → re-run
   `check`/`suggest-config` against `temp/plasma-bigscreen` (config at
   `temp/plasma-bigscreen/analyzer_config.toml`) to sanity-check real-world effect.
5. Commit style used throughout this branch: one commit for the code change(s) + tests,
   one separate commit for docs (README/AGENTS.md/CHANGELOG.md/docs file updates). Keep
   following that pattern for consistency; update `docs/cpp_projects_extension_ideas.md`'s
   checkboxes and CHANGELOG.md's `# Unreleased` section each time.
6. Not yet asked for by the user: merging this branch to `master`, or opening a PR. Don't do
   either without being asked.
