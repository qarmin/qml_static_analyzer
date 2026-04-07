use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process;

use qml_static_analyzer::checker::CheckContext;
use qml_static_analyzer::qt_types::QtTypeDb;
use qml_static_analyzer::types::FileItem;

// ── arg parsing ───────────────────────────────────────────────────────────────

pub struct CheckOpts {
    pub path: PathBuf,
    pub config_file: Option<PathBuf>,
    pub builtin_qt_version: Option<String>,
    pub qt_types_json: Option<PathBuf>,
    pub complex: bool,
    pub warn_useless_ignore: bool,
}

pub fn parse_check_args(args: &[String]) -> Result<CheckOpts, String> {
    let mut path: Option<PathBuf> = None;
    let mut config_file: Option<PathBuf> = None;
    let mut builtin_qt_version: Option<String> = None;
    let mut qt_types_json: Option<PathBuf> = None;
    let mut complex = false;
    let mut warn_useless_ignore = true;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--path" => {
                i += 1;
                path = Some(PathBuf::from(args.get(i).ok_or("missing value for --path")?));
            }
            "--config" => {
                i += 1;
                config_file = Some(PathBuf::from(args.get(i).ok_or("missing value for --config")?));
            }
            "--builtin-qt-version" => {
                i += 1;
                builtin_qt_version = Some(args.get(i).ok_or("missing value for --builtin-qt-version")?.clone());
            }
            "--qt-types-json" | "--qt-types" => {
                i += 1;
                qt_types_json = Some(PathBuf::from(args.get(i).ok_or("missing value for --qt-types-json")?));
            }
            "--complex" => {
                complex = true;
            }
            "--no-warn-useless-ignore" => {
                warn_useless_ignore = false;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    if builtin_qt_version.is_some() && qt_types_json.is_some() {
        return Err("--builtin-qt-version and --qt-types-json are mutually exclusive".to_string());
    }
    Ok(CheckOpts {
        path: path.ok_or("--path is required")?,
        config_file,
        builtin_qt_version,
        qt_types_json,
        complex,
        warn_useless_ignore,
    })
}

// ── subcommand: check ─────────────────────────────────────────────────────────

pub fn cmd_check(args: &[String]) {
    let opts = parse_check_args(args).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        eprintln!(
            "usage: qml_static_analyzer check --path <dir> \
             [--config <file>] [--builtin-qt-version X.Y.Z] [--qt-types-json <file>]"
        );
        process::exit(1);
    });

    if !opts.path.exists() {
        eprintln!("error: --path {:?} does not exist", opts.path);
        process::exit(1);
    }

    if let Some(ref p) = opts.config_file
        && !p.exists()
    {
        eprintln!("error: --config {p:?} does not exist");
        process::exit(1);
    }

    let config = opts
        .config_file
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| qml_static_analyzer::config::parse_config(&s))
        .unwrap_or_default();

    // Resolve the directory that contains the config file — headers are relative to it.
    let config_dir: std::path::PathBuf = opts
        .config_file
        .as_ref()
        .and_then(|p| p.parent())
        .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf);

    // Build cpp_object_members: name → None (opaque) | Some(known members).
    let mut cpp_object_members: HashMap<String, Option<HashSet<String>>> = HashMap::new();
    for (name, header_path) in &config.cpp_objects {
        let members = if header_path.is_empty() {
            None
        } else {
            let full_path = config_dir.join(header_path);
            match std::fs::read_to_string(&full_path) {
                Ok(src) => Some(qml_static_analyzer::cpp_header::parse_cpp_header(&src)),
                Err(e) => {
                    eprintln!("warning: cannot read header {full_path:?}: {e}");
                    None
                }
            }
        };
        cpp_object_members.insert(name.clone(), members);
    }

    // All C++ object names are available in QML scope.
    let mut cpp_globals: HashSet<String> = cpp_object_members.keys().cloned().collect();
    // [globals] names are always-valid identifiers.
    cpp_globals.extend(config.globals.names.iter().cloned());

    let db = load_qt_db(&opts);

    let all_qml_files = collect_qml_files(&opts.path);

    // Warn about ignore paths that don't match any file.
    for ignore_path in &config.ignore.paths {
        let matched = all_qml_files.iter().any(|p| {
            let relative = p.strip_prefix(&opts.path).unwrap_or(p);
            relative.starts_with(Path::new(ignore_path.as_str()))
        });
        if !matched {
            eprintln!(
                "warning: [ignore] path `{ignore_path}` does not match any .qml file \
                 (paths must be relative to --path)"
            );
        }
    }

    let qml_files: Vec<PathBuf> = all_qml_files
        .into_iter()
        .filter(|p| !is_ignored(p, &opts.path, &config.ignore.paths))
        .collect();

    if qml_files.is_empty() {
        eprintln!("no .qml files found in {:?}", opts.path);
        process::exit(1);
    }

    let mut parsed: Vec<(PathBuf, FileItem)> = Vec::new();
    let mut suppressed_lines: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
    for file_path in &qml_files {
        let source = std::fs::read_to_string(file_path).unwrap_or_else(|e| {
            eprintln!("cannot read {file_path:?}: {e}");
            process::exit(1);
        });
        let suppressed: HashSet<usize> = source
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("// qml-ignore"))
            .map(|(i, _)| i + 1)
            .collect();
        if !suppressed.is_empty() {
            suppressed_lines.insert(file_path.clone(), suppressed);
        }
        let name = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
        let file_item = qml_static_analyzer::parser::parse_file(name, &source).unwrap_or_else(|e| {
            eprintln!("parse error in {file_path:?}: {e}");
            process::exit(1);
        });
        parsed.push((file_path.clone(), file_item));
    }

    let known_types: HashSet<String> = parsed.iter().map(|(_, f)| f.name.clone()).collect();
    let file_members: HashMap<String, (Vec<String>, Vec<String>)> = parsed
        .iter()
        .map(|(_, f)| {
            let props: Vec<String> = f
                .properties
                .iter()
                .map(|p| p.name.clone())
                .chain(
                    f.functions
                        .iter()
                        .filter(|func| !func.is_signal_handler)
                        .map(|func| func.name.clone()),
                )
                .collect();
            let sigs: Vec<String> = f.signals.iter().map(|s| s.name.clone()).collect();
            (f.name.clone(), (props, sigs))
        })
        .collect();
    let file_base_types: HashMap<String, String> = parsed
        .iter()
        .map(|(_, f)| (f.name.clone(), f.base_type.clone()))
        .collect();

    let mut parent_scopes: HashMap<String, HashSet<String>> = HashMap::new();
    for (_, parent_file) in &parsed {
        let mut parent_names = collect_inherited_file_members(&parent_file.name, &file_members, &file_base_types);
        parent_names.extend(collect_all_file_ids(parent_file));
        let qt_base = resolve_qt_base(&parent_file.base_type, &db, &file_base_types);
        parent_names.extend(db.all_methods(&qt_base).keys().cloned());
        parent_names.extend(db.all_signals(&qt_base).keys().cloned());
        let mut all_children = collect_child_type_names(&parent_file.children);
        if let Some(extra) = config.new_child.get(&parent_file.name) {
            all_children.extend(extra.iter().cloned());
        }
        for child_type in all_children {
            if known_types.contains(&child_type) {
                parent_scopes
                    .entry(child_type)
                    .or_default()
                    .extend(parent_names.iter().cloned());
            }
        }
    }
    // For "ParentFile.loaderId" = ["ChildType"] config entries: the ChildType is loaded
    // *inside* the loader element, so it inherits the loader's own properties/signals.
    for (key, child_types) in &config.new_child {
        if let Some(dot_pos) = key.find('.') {
            let parent_name = &key[..dot_pos];
            let loader_id = &key[dot_pos + 1..];
            if let Some((_, parent_file)) = parsed.iter().find(|(_, f)| f.name == parent_name)
                && let Some(loader_type_name) = find_child_type_by_id(&parent_file.children, loader_id)
            {
                let loader_members = collect_inherited_file_members(&loader_type_name, &file_members, &file_base_types);
                if !loader_members.is_empty() {
                    for child_type in child_types {
                        if known_types.contains(child_type) {
                            parent_scopes
                                .entry(child_type.clone())
                                .or_default()
                                .extend(loader_members.iter().cloned());
                        }
                    }
                }
            }
        }
    }

    // Fixed-point propagation of parent scopes through the component hierarchy.
    loop {
        let mut changed = false;
        let snapshot = parent_scopes.clone();
        for (_, parent_file) in &parsed {
            let mut additional = snapshot.get(&parent_file.name).cloned().unwrap_or_default();
            additional.extend(collect_inherited_file_members(
                &parent_file.name,
                &file_members,
                &file_base_types,
            ));
            additional.extend(collect_all_file_ids(parent_file));
            let qt_base_iter = resolve_qt_base(&parent_file.base_type, &db, &file_base_types);
            additional.extend(db.all_methods(&qt_base_iter).keys().cloned());
            additional.extend(db.all_signals(&qt_base_iter).keys().cloned());
            if additional.is_empty() {
                continue;
            }
            let mut all_children = collect_child_type_names(&parent_file.children);
            if let Some(extra) = config.new_child.get(&parent_file.name) {
                all_children.extend(extra.iter().cloned());
            }
            for child_type in all_children {
                if known_types.contains(&child_type) {
                    let entry = parent_scopes.entry(child_type).or_default();
                    let before = entry.len();
                    entry.extend(additional.iter().cloned());
                    if entry.len() > before {
                        changed = true;
                    }
                }
            }
            // Also propagate through base-type relationships.
            let base = &parent_file.base_type;
            if file_members.contains_key(base) {
                let entry = parent_scopes.entry(base.clone()).or_default();
                let before = entry.len();
                entry.extend(additional.iter().cloned());
                if entry.len() > before {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Build a global id→type map for cross-file member access validation.
    let mut parent_id_types: HashMap<String, (String, Vec<qml_static_analyzer::types::Property>, Vec<String>, bool)> =
        HashMap::new();
    for (_, file) in &parsed {
        if let Some(id) = &file.id {
            let func_names: Vec<String> = file
                .functions
                .iter()
                .filter(|f| !f.is_signal_handler)
                .map(|f| f.name.clone())
                .chain(file.signals.iter().map(|s| s.name.clone()))
                .collect();
            parent_id_types
                .entry(id.clone())
                .or_insert_with(|| (file.base_type.clone(), file.properties.clone(), func_names, false));
        }
        collect_id_types_from_children(&file.children, &mut parent_id_types);
    }

    let check_ctx = CheckContext {
        known_types,
        extra_children: config.new_child.clone(),
        cpp_globals,
        cpp_object_members,
        file_members,
        file_base_types: file_base_types.clone(),
        parent_scopes,
        parent_id_types,
        complex: opts.complex,
    };

    let root_types: Vec<String> = parsed
        .iter()
        .filter(|(_, f)| {
            let qt_type = resolve_qt_base(&f.base_type, &db, &file_base_types);
            is_root_qt_type(&qt_type)
        })
        .map(|(_, f)| f.name.clone())
        .collect();

    let mut usage_paths: HashMap<String, Vec<String>> = HashMap::new();
    for root_type in &root_types {
        for (type_name, path) in build_usage_paths(&parsed, root_type, &config.new_child) {
            usage_paths.entry(type_name).or_insert(path);
        }
    }

    let mut total_errors = 0usize;
    let mut unreachable: Vec<&PathBuf> = Vec::new();

    for (file_path, file_item) in &parsed {
        if !usage_paths.contains_key(&file_item.name) {
            unreachable.push(file_path);
            continue;
        }

        let errors = qml_static_analyzer::checker::check_file(file_item, &db, &check_ctx);

        let usage_path = usage_paths.get(&file_item.name);

        let suppressed = suppressed_lines.get(file_path);
        let mut used_suppress_lines: HashSet<usize> = HashSet::new();
        for err in &errors {
            if let (Some(line), Some(sup)) = (err.line, suppressed)
                && sup.contains(&line)
            {
                used_suppress_lines.insert(line);
                continue;
            }
            let line_str = err.line.map(|l| format!(":{l}")).unwrap_or_default();
            let label = if opts.complex && !err.element_path.is_empty() {
                let combined: Vec<&str> = usage_path
                    .map(|p| p.iter().map(String::as_str).collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into_iter()
                    .chain(err.element_path.iter().map(String::as_str))
                    .collect();
                Some(format!(" [{}]", combined.join(" -> ")))
            } else {
                usage_path
                    .filter(|p| p.len() > 1)
                    .map(|p| format!(" [{}]", p.join(" -> ")))
            };
            if let Some(ref l) = label {
                println!("{}{line_str}: {err}{l}", file_path.display());
            } else {
                println!("{}{line_str}: {err}", file_path.display());
            }
        }
        total_errors += errors
            .iter()
            .filter(|e| {
                if let (Some(line), Some(sup)) = (e.line, suppressed) {
                    !sup.contains(&line)
                } else {
                    true
                }
            })
            .count();

        // Warn about `// qml-ignore` comments that didn't suppress any error.
        if opts.warn_useless_ignore
            && let Some(sup) = suppressed
        {
            let mut useless: Vec<usize> = sup.difference(&used_suppress_lines).copied().collect();
            useless.sort_unstable();
            for line in useless {
                println!(
                    "{}:{line}: Useless `// qml-ignore` — no error on this line",
                    file_path.display()
                );
                total_errors += 1;
            }
        }
    }

    if !unreachable.is_empty() {
        println!("\nNot checked (not reachable from any root):");
        for path in &unreachable {
            println!("  {}", path.display());
        }
    }

    println!("\nFound {total_errors} errors, {} files not checked", unreachable.len());

    if total_errors > 0 {
        process::exit(1);
    }
}

// ── Qt DB loading ─────────────────────────────────────────────────────────────

fn load_qt_db(opts: &CheckOpts) -> QtTypeDb {
    use qml_static_analyzer::qt_types;

    if let Some(ref path) = opts.qt_types_json {
        qt_types::load_from_json_file(path).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            process::exit(1);
        })
    } else if let Some(ref version) = opts.builtin_qt_version {
        qt_types::load_builtin_db(version).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            process::exit(1);
        })
    } else {
        if qt_types::builtin_versions().is_empty() {
            eprintln!(
                "error: no Qt types available. \
                 Use --builtin-qt-version or --qt-types-json, \
                 or re-build with INCLUDED_QT_TYPES=path/to/qt_types_X.Y.Z.json."
            );
            process::exit(1);
        }
        qt_types::load_default_builtin_db()
    }
}

// ── root discovery ────────────────────────────────────────────────────────────

const ROOT_QT_TYPES: &[&str] = &["Window", "ApplicationWindow", "Dialog"];

fn is_root_qt_type(qt_type: &str) -> bool {
    ROOT_QT_TYPES.contains(&qt_type)
}

/// Find the type name of a child element with the given id anywhere in the element tree.
fn find_child_type_by_id(children: &[qml_static_analyzer::types::QmlChild], id: &str) -> Option<String> {
    for child in children {
        if child.id.as_deref() == Some(id) {
            return Some(child.type_name.clone());
        }
        if let Some(found) = find_child_type_by_id(&child.children, id) {
            return Some(found);
        }
    }
    None
}

fn collect_id_types_from_children(
    children: &[qml_static_analyzer::types::QmlChild],
    map: &mut HashMap<String, (String, Vec<qml_static_analyzer::types::Property>, Vec<String>, bool)>,
) {
    for child in children {
        if let Some(id) = &child.id {
            let func_names: Vec<String> = child
                .functions
                .iter()
                .filter(|f| !f.is_signal_handler)
                .map(|f| f.name.clone())
                .chain(child.signals.iter().map(|s| s.name.clone()))
                .collect();
            map.entry(id.clone())
                .or_insert_with(|| (child.type_name.clone(), child.properties.clone(), func_names, child.is_loader_content));
        }
        collect_id_types_from_children(&child.children, map);
    }
}

fn collect_all_file_ids(file: &FileItem) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Some(id) = &file.id {
        ids.insert(id.clone());
    }
    fn recurse(children: &[qml_static_analyzer::types::QmlChild], ids: &mut HashSet<String>) {
        for child in children {
            if let Some(id) = &child.id {
                ids.insert(id.clone());
            }
            recurse(&child.children, ids);
        }
    }
    recurse(&file.children, &mut ids);
    ids
}

fn collect_inherited_file_members(
    type_name: &str,
    file_members: &HashMap<String, (Vec<String>, Vec<String>)>,
    file_base_types: &HashMap<String, String>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut current = type_name.to_string();
    let mut seen = HashSet::new();
    while seen.insert(current.clone()) {
        if let Some((props, sigs)) = file_members.get(&current) {
            names.extend(props.iter().cloned());
            names.extend(sigs.iter().cloned());
        }
        match file_base_types.get(&current) {
            Some(base) if file_members.contains_key(base.as_str()) => current = base.clone(),
            _ => break,
        }
    }
    names
}

fn resolve_qt_base(type_name: &str, db: &QtTypeDb, file_base_types: &HashMap<String, String>) -> String {
    let mut current = type_name.to_string();
    for _ in 0..32 {
        if db.has_type(&current) {
            return current;
        }
        match file_base_types.get(&current) {
            Some(base) => current = base.clone(),
            None => return current,
        }
    }
    current
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn is_ignored(file_path: &Path, base_dir: &Path, ignored_paths: &[String]) -> bool {
    let relative = file_path.strip_prefix(base_dir).unwrap_or(file_path);
    ignored_paths
        .iter()
        .any(|ignore| relative.starts_with(Path::new(ignore.as_str())))
}

pub fn build_usage_paths(
    parsed: &[(PathBuf, FileItem)],
    root_type: &str,
    extra_children: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let type_to_idx: HashMap<&str, usize> = parsed
        .iter()
        .enumerate()
        .map(|(i, (_, f))| (f.name.as_str(), i))
        .collect();

    let mut paths: HashMap<String, Vec<String>> = HashMap::new();
    if root_type.is_empty() || !type_to_idx.contains_key(root_type) {
        return paths;
    }

    paths.insert(root_type.to_string(), vec![root_type.to_string()]);
    let mut queue = VecDeque::new();
    queue.push_back(root_type.to_string());
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(root_type.to_string());

    while let Some(current) = queue.pop_front() {
        let current_path = paths[&current].clone();

        let mut child_types = Vec::new();
        if let Some(&idx) = type_to_idx.get(current.as_str()) {
            child_types.extend(collect_child_type_names(&parsed[idx].1.children));
            let base = &parsed[idx].1.base_type;
            if type_to_idx.contains_key(base.as_str()) {
                child_types.push(base.clone());
            }
        }
        if let Some(extra) = extra_children.get(&current) {
            child_types.extend(extra.iter().cloned());
        }

        for child_type in child_types {
            if type_to_idx.contains_key(child_type.as_str()) && !visited.contains(&child_type) {
                visited.insert(child_type.clone());
                let mut child_path = current_path.clone();
                child_path.push(child_type.clone());
                paths.insert(child_type.clone(), child_path);
                queue.push_back(child_type);
            }
        }
    }

    paths
}

fn collect_child_type_names(children: &[qml_static_analyzer::types::QmlChild]) -> Vec<String> {
    let mut result = Vec::new();
    for child in children {
        result.push(child.type_name.clone());
        result.extend(collect_child_type_names(&child.children));
    }
    result
}

fn collect_qml_files(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_qml_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("qml") {
            files.push(path);
        }
    }
    files.sort();
    files
}
