mod cmd_check;

use std::path::{Path, PathBuf};
use std::process;

// ── entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcmd = args.get(1).map(String::as_str);

    match subcmd {
        Some("--version") | Some("-V") => {
            println!("qml_static_analyzer {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help") | Some("-h") => {
            print_usage();
            return;
        }
        _ => eprintln!("qml_static_analyzer {}", env!("CARGO_PKG_VERSION")),
    }

    match subcmd {
        Some("check") => cmd_check::cmd_check(&args[2..]),
        Some("generate-qt-types") => cmd_generate_qt_types(&args[2..]),
        Some("list-builtins") => cmd_list_builtins(),
        Some(other) => {
            eprintln!("error: unknown subcommand '{other}'");
            print_usage();
            process::exit(1);
        }
        None => {
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("qml_static_analyzer {}", env!("CARGO_PKG_VERSION"));
    eprintln!("usage:");
    eprintln!("  qml_static_analyzer check --path <dir> [options]");
    eprintln!("    --config <file>               config file");
    eprintln!("    --builtin-qt-version <ver>    use compiled-in Qt types (e.g. 6.3.2)");
    eprintln!("    --qt-types-json <file>        use qt_types JSON from disk");
    eprintln!("    --complex                     show full element hierarchy in error paths");
    eprintln!("    --no-warn-useless-ignore      suppress warnings for useless // qml-ignore comments");
    eprintln!();
    eprintln!("  qml_static_analyzer generate-qt-types --qt-path <qt_gcc64_dir> [options]");
    eprintln!("    --output <file>               output file (default: qt_types_VER.json)");
    eprintln!("    --qt-version <ver>            override version string in output");
    eprintln!();
    eprintln!("  qml_static_analyzer list-builtins");
    eprintln!("    List Qt versions compiled into this binary.");
}

// ── subcommand: list-builtins ─────────────────────────────────────────────────

fn cmd_list_builtins() {
    let versions = qml_static_analyzer::qt_types::builtin_versions();
    if versions.is_empty() {
        println!("No Qt types compiled in.");
        println!("Re-build with INCLUDED_QT_TYPES=path/to/qt_types_X.Y.Z.json");
    } else {
        println!("Built-in Qt versions:");
        for v in versions {
            println!("  {v}");
        }
    }
}

// ── subcommand: generate-qt-types ────────────────────────────────────────────

struct GenerateOpts {
    qt_path: PathBuf,
    output: Option<PathBuf>,
    qt_version: Option<String>,
}

fn parse_generate_args(args: &[String]) -> Result<GenerateOpts, String> {
    let mut qt_path: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut qt_version: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--qt-path" => {
                i += 1;
                qt_path = Some(PathBuf::from(args.get(i).ok_or("missing value for --qt-path")?));
            }
            "--output" | "-o" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("missing value for --output")?));
            }
            "--qt-version" => {
                i += 1;
                qt_version = Some(args.get(i).ok_or("missing value for --qt-version")?.clone());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(GenerateOpts {
        qt_path: qt_path.ok_or("--qt-path is required")?,
        output,
        qt_version,
    })
}

fn cmd_generate_qt_types(args: &[String]) {
    let opts = parse_generate_args(args).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        eprintln!("usage: qml_static_analyzer generate-qt-types --qt-path <qt_gcc64_dir> [--output <file>] [--qt-version <ver>]");
        process::exit(1);
    });

    let version = opts
        .qt_version
        .clone()
        .or_else(|| detect_qt_version_from_path(&opts.qt_path))
        .unwrap_or_else(|| {
            eprintln!(
                "error: cannot detect Qt version from path. \
                 Add --qt-version X.Y.Z explicitly."
            );
            process::exit(1);
        });

    let json = qml_static_analyzer::qt_types_gen::generate_qt_types_json(&opts.qt_path).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });

    let json_with_version = inject_version_field(&json, &version).unwrap_or_else(|e| {
        eprintln!("error: failed to inject version into JSON: {e}");
        process::exit(1);
    });

    let out_path = opts
        .output
        .unwrap_or_else(|| PathBuf::from(format!("qt_types_{version}.json")));

    std::fs::write(&out_path, &json_with_version).unwrap_or_else(|e| {
        eprintln!("error: cannot write {out_path:?}: {e}");
        process::exit(1);
    });

    println!("Generated: {}", out_path.display());
    println!("Qt version: {version}");
    println!(
        "To embed in binary: INCLUDED_QT_TYPES={} cargo build",
        out_path.display()
    );
}

/// Try to extract a version string like `6.8.3` from a Qt install path.
fn detect_qt_version_from_path(path: &Path) -> Option<String> {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component
            && let Some(s) = name.to_str()
        {
            let parts: Vec<&str> = s.split('.').collect();
            if parts.len() >= 2
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
            {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Parse the JSON, add `"_version": version`, re-serialize.
fn inject_version_field(json: &str, version: &str) -> Result<String, String> {
    let mut obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    obj.insert("_version".to_string(), serde_json::Value::String(version.to_string()));
    serde_json::to_string_pretty(&obj).map_err(|e| e.to_string())
}
