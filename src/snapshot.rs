//! Snapshot helpers for integration tests.
//!
//! Format: RON (Rusty Object Notation) – zwięźlejszy od JSON, natywny dla Rusta.
//! Pliki snapshotów lądują obok pliku QML:
//!   `test_resources/<test_dir>/snapshot.ron`
//!
//! Zachowanie:
//! * Jeśli plik nie istnieje → zostaje **utworzony** (pierwszy run).
//! * Kolejne runy → snapshot jest **porównywany**; niezgodność = panika z diff-em RON.
//! * Aby zaakceptować zmianę: usuń plik `.ron` i uruchom testy ponownie.

use std::path::PathBuf;
use std::{env, fs};

use ron::ser::{PrettyConfig, to_string_pretty};
use serde::Serialize;
use serde::de::DeserializeOwned;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Ścieżka snapshotu dla `test_resources/<test_dir>/snapshot.ron`.
pub fn snapshot_path(test_dir: &str) -> PathBuf {
    manifest_dir()
        .join("test_resources")
        .join(test_dir)
        .join("snapshot.ron")
}

/// Ścieżka snapshotu dla testów inline → `test_resources/snapshots/<name>.ron`.
pub fn inline_snapshot_path(name: &str) -> PathBuf {
    manifest_dir()
        .join("test_resources")
        .join("snapshots")
        .join(format!("{name}.ron"))
}

fn to_ron<T: Serialize>(value: &T) -> String {
    let cfg = PrettyConfig::new()
        .depth_limit(10)
        .separate_tuple_members(false)
        .enumerate_arrays(false);
    to_string_pretty(value, cfg).expect("RON serialization failed")
}

/// Zapisuje `value` do pliku snapshotu (tworzy katalogi jeśli potrzeba).
pub fn save_snapshot<T: Serialize>(path: &std::path::Path, value: &T) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, to_ron(value)).unwrap_or_else(|e| panic!("Cannot write snapshot {}: {e}", path.display()));
}

/// Wczytuje snapshot; zwraca `None` gdy plik nie istnieje.
pub fn load_snapshot<T: DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    if !path.exists() {
        return None;
    }
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("Cannot read snapshot {}: {e}", path.display()));
    let val: T = ron::from_str(&text).unwrap_or_else(|e| panic!("Cannot deserialize snapshot {}: {e}", path.display()));
    Some(val)
}

/// Snapshot dla testu powiązanego z `test_resources/<test_dir>/`.
///
/// * Pierwszy run  → tworzy `snapshot.ron` i przechodzi.
/// * Kolejne runy  → porównuje; niezgodność = panika z diff-em RON.
pub fn assert_snapshot<T>(test_dir: &str, value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    run_snapshot_assert(&snapshot_path(test_dir), test_dir, value);
}

/// Snapshot dla testów wieloplikowych w `test_resources/<test_dir>/`.
/// Działa identycznie jak `assert_snapshot`, ale przyjmuje dowolny T
/// (np. `Vec<FileItem>`).
pub fn assert_dir_snapshot<T>(test_dir: &str, value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    run_snapshot_assert(&snapshot_path(test_dir), test_dir, value);
}

/// Snapshot dla testów inline (bez pliku QML).
pub fn assert_inline_snapshot<T>(name: &str, value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    run_snapshot_assert(&inline_snapshot_path(name), name, value);
}

#[expect(clippy::print_stdout)]
fn run_snapshot_assert<T>(path: &std::path::Path, label: &str, value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    match load_snapshot::<T>(path) {
        None => {
            save_snapshot(path, value);
            println!("📸  Snapshot created: {}", path.display());
        }
        Some(stored) => {
            if value != &stored {
                let current_ron = to_ron(value);
                let stored_ron = to_ron(&stored);
                panic!(
                    "\nSnapshot mismatch for `{label}`\n\
                     Jeśli zmiana jest zamierzona, usuń plik:\n  {}\n\
                     i uruchom testy ponownie.\n\n\
                     ── stored ──────────────────────────────\n{stored_ron}\n\
                     ── current ─────────────────────────────\n{current_ron}\n",
                    path.display()
                );
            }
        }
    }
}
