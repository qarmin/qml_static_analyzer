mod errors;
mod helpers;

pub use errors::{AnalysisError, ErrorKind};

use std::collections::{HashMap, HashSet};

use crate::parser::collect_dotted_accesses_from_expression;
use crate::qt_types::QtTypeDb;
use crate::types::{FileItem, Function, Property, PropertyType, PropertyValue, QmlChild};
use errors::AnalysisError as AE;
use helpers::{
    extract_import_aliases, handler_to_signal, is_inline_type_instantiation, is_js_global,
    prop_type_name, strip_changed_suffix, type_name_to_id, types_compatible, JS_GLOBALS,
    QML_DELEGATE_GLOBALS, QML_STYLE_GLOBALS,
};

// ── publiczne API ─────────────────────────────────────────────────────────

/// Kontekst przekazywany do checkera — informacje o projekcie spoza parsowanego pliku.
#[derive(Default)]
pub struct CheckContext {
    /// Typy plików QML które zostały sparsowane (nie są opaque).
    pub known_types: std::collections::HashSet<String>,
    /// Ręcznie zadeklarowane dzieci: parent_type → list of child_types.
    pub extra_children: std::collections::HashMap<String, Vec<String>>,
    /// Nazwy C++ dostępne globalnie w QML (singletony + obiekty kontekstu + globals).
    pub cpp_globals: std::collections::HashSet<String>,
    /// C++ object member sets: name → None (opaque, all access OK) | Some(members).
    /// Only objects listed here have their member access validated.
    pub cpp_object_members: std::collections::HashMap<String, Option<std::collections::HashSet<String>>>,
    /// Dla każdego znaneego typu QML: (nazwy property, nazwy sygnałów).
    /// Używane do dodawania własnych property/sygnałów do zasięgu danego dziecka.
    pub file_members: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// Dla każdego znaneego typu QML: typ bazowy (base_type z pliku .qml).
    /// Używane do pobierania właściwości Qt dla znanych typów (np. Sub2 → Switch → checked).
    pub file_base_types: std::collections::HashMap<String, String>,
    /// Dla każdego typu QML: nazwy property i sygnałów dostępnych z zasięgu rodzica (parent scope).
    /// Zapobiega fałszywym błędom dla referencji do property zdefiniowanych w pliku nadrzędnym.
    pub parent_scopes: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// Global id → type info map: element ids (and their types) visible across files.
    /// Allows validating `mainWindow.propName` accesses even in child files that don't own mainWindow.
    /// Maps id → (type_name, declared_properties, function_names, is_loader_content).
    pub parent_id_types: std::collections::HashMap<String, (String, Vec<crate::types::Property>, Vec<String>, bool)>,
    /// When true, errors carry a full within-file element path (for --complex output mode).
    pub complex: bool,
}

impl CheckContext {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub fn check_file(file: &FileItem, db: &QtTypeDb, ctx: &CheckContext) -> Vec<AnalysisError> {
    let mut errors = Vec::new();
    let checker = Checker { db, ctx };

    // Zbuduj zestaw nazw zadeklarowanych na poziomie pliku (property + sygnały + id)
    let file_scope = checker.build_file_scope(file);

    checker.check_root(file, &file_scope, &mut errors);
    errors
}

// ── wewnętrzna logika ─────────────────────────────────────────────────────

struct Checker<'a> {
    db: &'a QtTypeDb,
    ctx: &'a CheckContext,
}

/// Info o dziecku potrzebne do sprawdzania member access w funkcjach.
#[derive(Clone)]
struct ChildInfo {
    type_name: String,
    properties: Vec<Property>,
    /// Names of functions declared inline in this child instantiation (not the type def).
    function_names: Vec<String>,
    /// Names of signals declared inline in this child instantiation.
    signal_names: Vec<String>,
    is_loader_content: bool,
}

fn build_child_id_map(children: &[QmlChild]) -> HashMap<String, ChildInfo> {
    let mut map = HashMap::new();
    for child in children {
        if let Some(id) = &child.id {
            map.insert(
                id.clone(),
                ChildInfo {
                    type_name: child.type_name.clone(),
                    properties: child.properties.clone(),
                    function_names: child.functions.iter().map(|f| f.name.clone()).collect(),
                    signal_names: child.signals.iter().map(|s| s.name.clone()).collect(),
                    is_loader_content: child.is_loader_content,
                },
            );
        }
        // Rekurencyjnie — id zagnieżdżonych dzieci też są widoczne
        let nested = build_child_id_map(&child.children);
        map.extend(nested);
    }
    map
}

impl Checker<'_> {
    // ── rozwiązywanie łańcucha typów bazowych ────────────────────────────

    /// Classify a type name that may contain a module alias prefix.
    ///
    /// Three outcomes (encoded as `Option<(&str, bool)>` where the `bool` means
    /// "full validation allowed"):
    ///
    /// * `None`              – prefix is **not** a known import alias → fall through to
    ///                         normal `UnknownType` check.
    ///
    /// * `Some((name, true))`  – Qt-module alias (`import QtQuick.Controls as QQC2`) AND
    ///                           bare `name` is in the Qt DB → **full** validation.
    ///
    /// * `Some((name, false))` – non-Qt alias (e.g. `org.kde.*`) AND bare `name` is in
    ///                           the Qt DB → use for scope/signal lookup but **skip**
    ///                           `UnknownPropertyAssignment` (the external type may expose
    ///                           additional properties not in the Qt type).
    ///                           Also used when bare name is NOT in DB → `name` == original
    ///                           dotted string → caller should `return` (opaque external type).
    fn resolve_aliased_base<'a>(&self, type_name: &'a str, aliases: &HashMap<String, bool>) -> Option<(&'a str, bool)> {
        if let Some(dot_pos) = type_name.find('.') {
            let prefix = &type_name[..dot_pos];
            if let Some(&is_qt) = aliases.get(prefix) {
                let unqualified = &type_name[dot_pos + 1..];
                if self.db.has_type(unqualified) || self.ctx.known_types.contains(unqualified) {
                    // Resolved: full validation for Qt aliases, partial for non-Qt
                    return Some((unqualified, is_qt));
                }
                // Known alias but type not in DB → opaque external type
                return Some((type_name, false));
            }
        }
        None // not an alias prefix
    }

    /// Zwraca najbliższy typ Qt w łańcuchu typów bazowych.
    /// Np. TextButton → GenericButton → RoundButton (Qt type).
    /// Zapobiega nieskończonej pętli przez max 32 kroki.
    fn resolve_qt_type(&self, type_name: &str) -> String {
        let mut current = type_name.to_string();
        for _ in 0..32 {
            if self.db.has_type(&current) {
                return current;
            }
            match self.ctx.file_base_types.get(&current) {
                Some(base) => current = base.clone(),
                None => return current,
            }
        }
        current
    }

    // ── inherited file members ────────────────────────────────────────────

    /// Collects all (prop, sig) names declared in a type's QML base chain.
    /// E.g. Sub4 → SwitchWrapper → returns props from both files.
    fn all_file_member_names(&self, type_name: &str) -> HashSet<String> {
        let mut names = HashSet::new();
        let mut current = type_name.to_string();
        let mut seen = HashSet::new();
        while seen.insert(current.clone()) {
            if let Some((props, sigs)) = self.ctx.file_members.get(&current) {
                names.extend(props.iter().cloned());
                names.extend(sigs.iter().cloned());
            }
            match self.ctx.file_base_types.get(&current) {
                Some(b) if self.ctx.file_members.contains_key(b.as_str()) => current = b.clone(),
                _ => break,
            }
        }
        names
    }

    // ── budowanie zasięgu ─────────────────────────────────────────────────

    /// Wszystkie nazwy widoczne na poziomie root FileItem.
    fn build_file_scope(&self, file: &FileItem) -> HashSet<String> {
        let mut scope = HashSet::new();
        // Wbudowane JS globals
        for g in JS_GLOBALS {
            scope.insert(g.to_string());
        }
        // QML delegate globals (model, modelData, index)
        for g in QML_DELEGATE_GLOBALS {
            scope.insert(g.to_string());
        }
        // QML style attached-type globals (Material, Universal, Imagine)
        for g in QML_STYLE_GLOBALS {
            scope.insert(g.to_string());
        }
        // `parent` is always available in QML (refers to the visual parent element).
        // The actual parent type varies by usage context and cannot be determined statically,
        // so we only ensure the name is in scope without validating member access on it.
        scope.insert("parent".to_string());
        // Aliasy importów: `import "..." as Alias` → Alias jest dostępne globalnie
        // Also: `import TypeName 1.0` → TypeName is a C++ registered type/enum module
        for import_str in &file.imports {
            if let Some(as_pos) = import_str.find(" as ") {
                let alias = import_str[as_pos + 4..].trim();
                if !alias.is_empty()
                    && alias.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                    && alias.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    scope.insert(alias.to_string());
                }
            } else {
                // `import TypeName 1.0` — module name itself becomes accessible as a namespace
                let parts: Vec<&str> = import_str.split_whitespace().collect();
                if parts.len() >= 3
                    && parts[0] == "import"
                    && !parts[1].starts_with('"')
                    && parts[1].chars().next().is_some_and(|c| c.is_uppercase())
                {
                    scope.insert(parts[1].to_string());
                }
            }
        }
        // id root elementu
        if let Some(id) = &file.id {
            scope.insert(id.clone());
        }
        // property root + auto-generated <propName>Changed signals
        for p in &file.properties {
            scope.insert(p.name.clone());
            scope.insert(format!("{}Changed", p.name));
        }
        // sygnały root
        for s in &file.signals {
            scope.insert(s.name.clone());
        }
        // funkcje root
        for f in &file.functions {
            scope.insert(f.name.clone());
        }
        // id dzieci
        self.collect_child_ids(&file.children, &mut scope);
        // property z bazy Qt (przez pełen łańcuch typów bazowych)
        // Resolve aliased base type first:
        //   `QQC2.ToolButton`      → Some(("ToolButton", true))  → full Qt props in scope
        //   `Kirigami.Page`        → Some(("Page", false))       → Qt props for scope (partial)
        //   `Kirigami.CustomThing` → Some(("Kirigami.CustomThing", false)) → opaque
        //   `Rectangle`            → None                         → normal lookup
        let import_aliases = extract_import_aliases(&file.imports);
        let effective_base_name: &str = match self.resolve_aliased_base(&file.base_type, &import_aliases) {
            // Opaque: alias prefix known but type not in DB → name unchanged → no Qt props
            Some((name, _)) if name == file.base_type => &file.base_type,
            Some((name, _)) => name,
            None => &file.base_type,
        };
        let resolved_base = self.resolve_qt_type(effective_base_name);
        for (name, _) in self.db.all_properties(&resolved_base) {
            scope.insert(name);
        }
        let qt_methods = self.db.all_methods(&resolved_base);
        for name in qt_methods.keys() {
            scope.insert(name.clone());
        }
        // Qt signals of the root base type are also callable (to emit them).
        let qt_sigs_base = self.db.all_signals(&resolved_base);
        for name in qt_sigs_base.keys() {
            scope.insert(name.clone());
        }
        // Properties/signals from user-defined QML base type chain
        // e.g. Global extends WindowBase → windowBusy from WindowBase is in scope
        {
            let mut current = effective_base_name.to_string();
            let mut seen = HashSet::new();
            while seen.insert(current.clone()) {
                if let Some((props, sigs)) = self.ctx.file_members.get(&current) {
                    for p in props {
                        scope.insert(p.clone());
                    }
                    for s in sigs {
                        scope.insert(s.clone());
                    }
                }
                match self.ctx.file_base_types.get(&current) {
                    Some(b) if self.ctx.file_members.contains_key(b.as_str()) => current = b.clone(),
                    _ => break,
                }
            }
        }
        // Property dostępne z zasięgu rodzica (pliku nadrzędnego który zawiera ten typ).
        // Np. Sub4.qml może odwoływać się do property zdefiniowanych w Global.qml.
        if let Some(parent_scope) = self.ctx.parent_scopes.get(&file.name) {
            for name in parent_scope {
                // Also synthesise the auto-generated Changed signal so that
                // `worklistDataChanged()` is valid when `worklistData` lives in a
                // parent file and reaches this file's scope via parent_scopes.
                scope.insert(format!("{}Changed", name));
                scope.insert(name.clone());
            }
        }
        // Syntetyczne id dzieci z konfiguracji (new_child)
        // The config supports two forms of keys:
        //  - ParentType = ["ChildType1", ...]         -> applies file-wide (creates synthetic child ids from child types)
        //  - ParentType.childId = ["ChildType1", ...] -> attaches child types to a specific child id inside the file
        if let Some(child_types) = self.ctx.extra_children.get(&file.name) {
            for child_type in child_types {
                scope.insert(type_name_to_id(child_type));
                // Also expose that child type's properties and signals
                if let Some((props, sigs)) = self.ctx.file_members.get(child_type) {
                    for prop in props {
                        scope.insert(prop.clone());
                    }
                    for sig in sigs {
                        scope.insert(sig.clone());
                    }
                }
            }
        }
        // per-instance entries: keys like "FileName.childId"
        let prefix = format!("{}.", file.name);
        for (key, child_types) in &self.ctx.extra_children {
            if key.starts_with(&prefix) {
                let child_id = key[prefix.len()..].to_string();
                // expose the child id itself and its members
                scope.insert(child_id.clone());
                for child_type in child_types {
                    if let Some((props, sigs)) = self.ctx.file_members.get(child_type) {
                        for prop in props {
                            scope.insert(prop.clone());
                        }
                        for sig in sigs {
                            scope.insert(sig.clone());
                        }
                    }
                }
            }
        }
        // C++ globale (singletony + obiekty kontekstu)
        for name in &self.ctx.cpp_globals {
            scope.insert(name.clone());
        }
        scope
    }

    fn collect_child_ids(&self, children: &[QmlChild], scope: &mut HashSet<String>) {
        for child in children {
            if let Some(id) = &child.id {
                scope.insert(id.clone());
            }
            self.collect_child_ids(&child.children, scope);
        }
    }

    // ── sprawdzanie root ──────────────────────────────────────────────────

    fn check_root(&self, file: &FileItem, file_scope: &HashSet<String>, errors: &mut Vec<AE>) {
        // Resolve aliased base type.
        // `resolve_aliased_base` returns:
        //   None                           → not an alias, look up normally
        //   Some(("ToolButton",  true))    → Qt alias + in DB → full validation
        //   Some(("Page",        false))   → non-Qt alias + in DB → scope OK, skip assignment checks
        //   Some(("Kirigami.X",  false))   → alias but NOT in DB  → opaque (name unchanged)
        let import_aliases = extract_import_aliases(&file.imports);
        let (effective_base_name, base_full_validation): (&str, bool) =
            match self.resolve_aliased_base(&file.base_type, &import_aliases) {
                // Opaque: alias prefix known but type not in DB → name unchanged
                Some((name, _)) if name == file.base_type => (&file.base_type, false),
                Some((name, full)) => (name, full),
                None => (&file.base_type, true), // not an alias → normal validation
            };
        let effective_base = self.resolve_qt_type(effective_base_name);
        let qt_props = self.db.all_properties(&effective_base);
        let qt_signals = self.db.all_signals(&effective_base);

        // Skip UnknownPropertyAssignment when:
        //   - non-Qt aliased root (may expose more props than the Qt type), OR
        //   - opaque aliased root (bare name not in DB, qt_props is empty).
        let base_is_opaque = !base_full_validation;

        // Budujemy mapę property pliku (name → PropertyType) do sprawdzania referencji
        let file_prop_types: HashMap<String, &PropertyType> =
            file.properties.iter().map(|p| (p.name.clone(), &p.prop_type)).collect();

        // Mapa id dzieci → info o dziecku (do sprawdzania member access w funkcjach)
        let mut child_id_map = build_child_id_map(&file.children);

        // Add root element id so that `rootId.member = value` accesses can be validated.
        // Include both function names AND declared signal names (signals are callable members).
        if let Some(root_id) = &file.id {
            child_id_map.entry(root_id.clone()).or_insert_with(|| ChildInfo {
                type_name: file.base_type.clone(),
                properties: file.properties.clone(),
                function_names: file
                    .functions
                    .iter()
                    .map(|f| f.name.clone())
                    .chain(file.signals.iter().map(|s| s.name.clone()))
                    .collect(),
                signal_names: file.signals.iter().map(|s| s.name.clone()).collect(),
                is_loader_content: false,
            });
        }

        // Globally-visible element ids from parent files (e.g. mainWindow from main.qml).
        // Added with or_insert_with so local children take precedence.
        // Skip any id that is also a locally-declared property: a `property var foo`
        // (or any typed property) shadows a global id of the same name, and since
        // the property type may be unknown (var), member-access validation must be
        // skipped entirely.
        let local_prop_names: HashSet<&str> = file.properties.iter().map(|p| p.name.as_str()).collect();
        for (id, (type_name, properties, function_names, is_loader_content)) in &self.ctx.parent_id_types {
            if local_prop_names.contains(id.as_str()) {
                continue;
            }
            child_id_map.entry(id.clone()).or_insert_with(|| ChildInfo {
                type_name: type_name.clone(),
                properties: properties.clone(),
                function_names: function_names.clone(),
                signal_names: vec![],
                is_loader_content: *is_loader_content,
            });
        }

        // Syntetyczne dzieci z konfiguracji (new_child)
        if let Some(child_types) = self.ctx.extra_children.get(&file.name) {
            for child_type in child_types {
                let id = type_name_to_id(child_type);
                child_id_map.entry(id).or_insert_with(|| ChildInfo {
                    type_name: child_type.clone(),
                    properties: vec![],
                    function_names: vec![],
                    signal_names: vec![],
                    is_loader_content: false,
                });
            }
        }
        // per-instance extra_children entries (ParentType.childId)
        let prefix = format!("{}.", file.name);
        for (key, child_types) in &self.ctx.extra_children {
            if key.starts_with(&prefix) {
                let child_id = key[prefix.len()..].to_string();
                for child_type in child_types {
                    child_id_map.entry(child_id.clone()).or_insert_with(|| ChildInfo {
                        type_name: child_type.clone(),
                        properties: vec![],
                        function_names: vec![],
                        signal_names: vec![],
                        is_loader_content: false,
                    });
                }
            }
        }

        // NOTE: `parent` is intentionally NOT added to child_id_map. In QML, `parent`
        // refers to the visual parent whose type varies by instantiation context and
        // cannot be determined statically. Adding it as `Item` caused false positives
        // (e.g. `parent.radius` on a Rectangle parent, `parent.goToReportsScreen` on a
        // custom component). `parent` is in scope via build_file_scope; member access
        // on it is simply not validated.

        // Resolve property aliases: `property alias X: Y` — if Y is a known child id,
        // make X point to the same ChildInfo as Y. This overrides any conflicting entry
        // from parent_id_types (e.g. another file's root element that happens to have the
        // same id as the alias name).
        for prop in &file.properties {
            if prop.is_simple_ref
                && matches!(prop.prop_type, PropertyType::Custom(ref t) if t == "alias")
                && !prop.accessed_properties.is_empty()
            {
                let target = &prop.accessed_properties[0];
                if let Some(info) = child_id_map.get(target).cloned() {
                    child_id_map.insert(prop.name.clone(), info);
                }
            }
        }

        // 1. Sprawdź deklaracje property
        for prop in &file.properties {
            Self::check_property_decl(prop, &qt_props, &file_prop_types, file_scope, self.db, errors, None);
        }

        // 1b. C++ and QML member validation for property declaration expressions.
        // `check_property_decl` only checks scope via `accessed_properties` (base names).
        // We additionally validate `base.member` accesses against `cpp_object_members` and
        // known QML child ids using the raw expression string stored at parse time.
        let mut seen_qml_prop_decl: HashSet<(String, String)> = HashSet::new();
        for prop in &file.properties {
            if prop.raw_value_expr.is_empty() {
                continue;
            }
            let mut seen_cpp: HashSet<(String, String)> = HashSet::new();
            for (base_name, member_opt) in collect_dotted_accesses_from_expression(&prop.raw_value_expr) {
                let Some(accessed) = member_opt else { continue };
                let key = (base_name.clone(), accessed.clone());
                if seen_cpp.insert(key.clone())
                    && let Some(members_opt) = self.ctx.cpp_object_members.get(&base_name)
                    && let Some(members) = members_opt
                    && !members.contains(accessed.as_str())
                {
                    errors.push(
                        AE::new(ErrorKind::UnknownCppMember {
                            object: base_name.clone(),
                            member: accessed.clone(),
                        })
                        .with_line(prop.line),
                    );
                }
                // Also validate QML child member access in property declaration expressions.
                if !self.ctx.cpp_object_members.contains_key(&base_name) {
                    if let Some(child_info2) = child_id_map.get(&base_name) {
                        if child_info2.type_name != "Connections" {
                            let child_member_names = self.all_file_member_names(&child_info2.type_name);
                            let qt_base2 = self.resolve_qt_type(&child_info2.type_name);
                            if !child_member_names.is_empty()
                                || !child_info2.properties.is_empty()
                                || self.db.has_type(&child_info2.type_name)
                                || self.db.has_type(&qt_base2)
                            {
                                let qt_child_props2 = self.db.all_properties(&qt_base2);
                                let qt_child_sigs2 = self.db.all_signals(&qt_base2);
                                let qt_child_methods2 = self.db.all_methods(&qt_base2);
                                let loader_methods2 = if child_info2.is_loader_content {
                                    self.db.all_methods("Loader")
                                } else {
                                    HashMap::new()
                                };
                                // Also allow Loader's own properties (e.g. `item`, `status`) when
                                // the child id belongs to a Loader whose content is the proxy type.
                                let loader_own_props2 = if child_info2.is_loader_content {
                                    self.db.all_properties("Loader")
                                } else {
                                    HashMap::new()
                                };
                                let is_auto_sig = accessed.strip_suffix("Changed").is_some_and(|base| {
                                    child_member_names.contains(base)
                                        || qt_child_props2.contains_key(base)
                                        || child_info2.properties.iter().any(|p| p.name == base)
                                });
                                let member_valid = child_member_names.contains(accessed.as_str())
                                    || qt_child_props2.contains_key(accessed.as_str())
                                    || qt_child_sigs2.contains_key(accessed.as_str())
                                    || qt_child_methods2.contains_key(accessed.as_str())
                                    || loader_methods2.contains_key(accessed.as_str())
                                    || loader_own_props2.contains_key(accessed.as_str())
                                    || child_info2.properties.iter().any(|p| p.name == accessed.as_str())
                                    || child_info2.function_names.iter().any(|f| f == accessed.as_str())
                                    || is_auto_sig
                                    // For `parent`, all Item properties are always valid because any
                                    // visual element is Item-derived and anchor/size properties are universal.
                                    || (base_name == "parent"
                                        && self.db.all_properties("Item").contains_key(accessed.as_str()));
                                if !member_valid && seen_qml_prop_decl.insert(key) {
                                    errors.push(
                                        AE::new(ErrorKind::UnknownQmlMember {
                                            object: base_name.clone(),
                                            member: accessed.clone(),
                                        })
                                        .with_line(prop.line),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Sprawdź funkcje / handlery sygnałów
        for func in &file.functions {
            self.check_function(
                func,
                file_scope,
                &qt_props,
                &qt_signals,
                &file.signals,
                &child_id_map,
                errors,
                None,
            );
        }

        // 2b. Check JS-block property value bodies (e.g. `text: { if (foo) ... }`).
        for func in &file.property_js_block_funcs {
            self.check_function(func, file_scope, &qt_props, &qt_signals, &file.signals, &child_id_map, errors, None);
        }

        // 3. Sprawdź root-level inline assignments (e.g. `width: expr`, `invalidProp: val`).
        // Skip entirely for opaque aliased base types — we don't know their property set.
        let type_member_names = self.all_file_member_names(&file.name);
        let mut seen_qml_assign_root: HashSet<(String, String)> = HashSet::new();
        for (name, value_expr, line) in &file.assignments {
            if base_is_opaque {
                continue;
            }
            // Skip the property-name check for dotted keys (e.g. `icon.source`): these are
            // grouped/attached properties that are not individually listed in the Qt DB, so we
            // cannot validate the key itself — but we still validate the RHS below.
            let prop_known = name.contains('.')
                || qt_props.contains_key(name.as_str())
                || file.properties.iter().any(|p| p.name == *name)
                || type_member_names.contains(name.as_str());
            if !prop_known {
                errors.push(
                    AE::new(ErrorKind::UnknownPropertyAssignment { name: name.clone() }).with_line(*line),
                );
            }
            // Check names in the value expression against scope.
            // Skip standalone object literals `{…}` and inline type instantiations `TypeName {…}`.
            if !is_inline_type_instantiation(value_expr) {
                let mut seen_names: HashSet<String> = HashSet::new();
                let mut seen_cpp: HashSet<(String, String)> = HashSet::new();
                for (base_name, member) in collect_dotted_accesses_from_expression(value_expr) {
                    if let Some(accessed) = &member {
                        let key = (base_name.clone(), accessed.clone());
                        if seen_cpp.insert(key)
                            && let Some(members_opt) = self.ctx.cpp_object_members.get(&base_name)
                            && let Some(members) = members_opt
                            && !members.contains(accessed.as_str())
                        {
                            errors.push(
                                AE::new(ErrorKind::UnknownCppMember {
                                    object: base_name.clone(),
                                    member: accessed.clone(),
                                })
                                .with_line(*line),
                            );
                        }
                    }

                    // Also validate QML child member access.
                    if let Some(accessed) = &member {
                        if !self.ctx.cpp_object_members.contains_key(&base_name) {
                            if let Some(child_info2) = child_id_map.get(&base_name) {
                                if child_info2.type_name != "Connections" {
                                    let child_member_names = self.all_file_member_names(&child_info2.type_name);
                                    let qt_base2 = self.resolve_qt_type(&child_info2.type_name);
                                    if !child_member_names.is_empty()
                                        || !child_info2.properties.is_empty()
                                        || self.db.has_type(&child_info2.type_name)
                                        || self.db.has_type(&qt_base2)
                                    {
                                        let qt_child_props2 = self.db.all_properties(&qt_base2);
                                        let qt_child_sigs2 = self.db.all_signals(&qt_base2);
                                        let qt_child_methods2 = self.db.all_methods(&qt_base2);
                                        let loader_methods2 = if child_info2.is_loader_content {
                                            self.db.all_methods("Loader")
                                        } else {
                                            HashMap::new()
                                        };
                                        let loader_own_props2 = if child_info2.is_loader_content {
                                            self.db.all_properties("Loader")
                                        } else {
                                            HashMap::new()
                                        };
                                        let is_auto_sig =
                                            accessed.strip_suffix("Changed").is_some_and(|base| {
                                                child_member_names.contains(base)
                                                    || qt_child_props2.contains_key(base)
                                                    || child_info2.properties.iter().any(|p| p.name == base)
                                            });
                                        let member_valid = child_member_names.contains(accessed.as_str())
                                            || qt_child_props2.contains_key(accessed.as_str())
                                            || qt_child_sigs2.contains_key(accessed.as_str())
                                            || qt_child_methods2.contains_key(accessed.as_str())
                                            || loader_methods2.contains_key(accessed.as_str())
                                            || loader_own_props2.contains_key(accessed.as_str())
                                            || child_info2.properties.iter().any(|p| p.name == accessed.as_str())
                                            || child_info2.function_names.iter().any(|f| f == accessed.as_str())
                                            || is_auto_sig
                                            || (base_name == "parent"
                                                && self.db.all_properties("Item").contains_key(accessed.as_str()));
                                        if !member_valid {
                                            let seen_key = (base_name.clone(), accessed.clone());
                                            if seen_qml_assign_root.insert(seen_key) {
                                                errors.push(
                                                    AE::new(ErrorKind::UnknownQmlMember {
                                                        object: base_name.clone(),
                                                        member: accessed.clone(),
                                                    })
                                                    .with_line(*line),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !seen_names.insert(base_name.clone()) {
                        continue;
                    }
                    if self.db.has_type(&base_name) {
                        continue;
                    }
                    if !file_scope.contains(base_name.as_str()) && !is_js_global(&base_name) {
                        errors.push(
                            AE::new(ErrorKind::UndefinedPropertyAccess {
                                prop: name.clone(),
                                name: base_name.clone(),
                            })
                            .with_line(*line),
                        );
                    }
                }
            }
        }

        // 4. Sprawdź dzieci
        // For direct children of this file, their QML `parent` is the root element.
        // Build a parent-aware id map so `parent.xxx` accesses can be validated against
        // the actual root element's type instead of being skipped entirely.
        let mut child_id_map_for_children = child_id_map.clone();
        child_id_map_for_children.entry("parent".to_string()).or_insert_with(|| ChildInfo {
            type_name: file.name.clone(),
            properties: file.properties.clone(),
            function_names: file
                .functions
                .iter()
                .filter(|f| !f.is_signal_handler)
                .map(|f| f.name.clone())
                .chain(file.signals.iter().map(|s| s.name.clone()))
                .collect(),
            signal_names: file.signals.iter().map(|s| s.name.clone()).collect(),
            is_loader_content: false,
        });
        for child in &file.children {
            self.check_child(child, file_scope, errors, &[], &child_id_map_for_children, &import_aliases);
        }
    }

    // ── sprawdzanie property ──────────────────────────────────────────────

    fn check_property_decl(
        prop: &Property,
        qt_props: &HashMap<String, String>,
        file_prop_types: &HashMap<String, &PropertyType>,
        scope: &HashSet<String>,
        db: &QtTypeDb,
        errors: &mut Vec<AE>,
        context: Option<&str>,
    ) {
        // 1. Redefinicja property z klasy bazowej
        if qt_props.contains_key(&prop.name) {
            let base = "base type".to_string();
            let mut e = AE::new(ErrorKind::PropertyRedefinition {
                name: prop.name.clone(),
                base_type: base,
            })
            .with_line(prop.line);
            if let Some(c) = context {
                e = e.with_context(c);
            }
            errors.push(e);
        }

        // 2. Niezgodność typu literału
        if !matches!(prop.prop_type, PropertyType::Var)
            && let Some(mismatch) = Self::literal_type_mismatch(&prop.prop_type, &prop.value)
        {
            let mut e = AE::new(ErrorKind::PropertyTypeMismatch {
                name: prop.name.clone(),
                declared: prop_type_name(&prop.prop_type),
                assigned: mismatch,
            })
            .with_line(prop.line);
            if let Some(c) = context {
                e = e.with_context(c);
            }
            errors.push(e);
        }

        // 3. Niezgodność przez referencję do innej property (TooComplex = skip)
        if matches!(prop.value, PropertyValue::TooComplex) {
            // wyrażenie — nie sprawdzamy
        } else if prop.value == PropertyValue::Unset {
            // brak wartości — ok
        } else {
            // sprawdź accessed_properties – jeśli jest dokładnie jedna i wartość to TooComplex -> nie
            // (obsługiwane wyżej)
        }

        // Jeśli wartość to identyfikator innej property (prosty ref, np. `property bool b: other`)
        // – sprawdź zgodność typów. Dla złożonych wyrażeń (np. `!other`) pomijamy.
        if matches!(prop.value, PropertyValue::TooComplex)
            && prop.is_simple_ref
            && !matches!(prop.prop_type, PropertyType::Var)
        {
            let ref_name = &prop.accessed_properties[0];
            if let Some(ref_type) = file_prop_types.get(ref_name)
                && !types_compatible(&prop.prop_type, ref_type)
            {
                let mut e = AE::new(ErrorKind::PropertyRefTypeMismatch {
                    name: prop.name.clone(),
                    declared: prop_type_name(&prop.prop_type),
                    ref_name: ref_name.clone(),
                    ref_type: prop_type_name(ref_type),
                })
                .with_line(prop.line);
                if let Some(c) = context {
                    e = e.with_context(c);
                }
                errors.push(e);
            }
        }

        // Sprawdź czy wszystkie nazwy w wyrażeniu property są w zasięgu
        if matches!(prop.value, PropertyValue::TooComplex) {
            let mut seen = HashSet::new();
            for name in &prop.accessed_properties {
                if !seen.insert(name.as_str()) {
                    continue; // zdeduplikuj
                }
                if db.has_type(name) {
                    continue; // Qt type used as enum namespace (e.g. `Popup.CloseOnEscape`)
                }
                if !scope.contains(name.as_str()) {
                    let mut e = AE::new(ErrorKind::UndefinedPropertyAccess {
                        prop: prop.name.clone(),
                        name: name.clone(),
                    })
                    .with_line(prop.line);
                    if let Some(c) = context {
                        e = e.with_context(c);
                    }
                    errors.push(e);
                }
            }
        }
    }

    /// Zwraca Some("bool"/"int"/…) jeśli literał nie pasuje do deklarowanego typu.
    fn literal_type_mismatch(declared: &PropertyType, value: &PropertyValue) -> Option<String> {
        // early-return for non-literals / null
        match value {
            PropertyValue::Unset | PropertyValue::TooComplex | PropertyValue::Null => return None,
            _ => {}
        }

        match (declared, value) {
            // assigned a bool where a non-bool numeric/string was expected
            (
                PropertyType::Int | PropertyType::String | PropertyType::Double | PropertyType::Real,
                PropertyValue::Bool(_),
            ) => Some("bool".into()),
            // assigned an int where a bool or string was expected
            (PropertyType::Bool | PropertyType::String, PropertyValue::Int(_)) => Some("int".into()),
            // assigned a double where a bool or int was expected
            (PropertyType::Bool | PropertyType::Int, PropertyValue::Double(_)) => Some("double".into()),
            // assigned a string literal where a numeric or bool type was expected
            (
                PropertyType::Int | PropertyType::Bool | PropertyType::Double | PropertyType::Real,
                PropertyValue::String(_),
            ) => Some("string".into()),
            // assigned a double literal where string was expected
            (PropertyType::String, PropertyValue::Double(_)) => Some("double".into()),
            _ => None,
        }
    }

    // ── sprawdzanie funkcji ───────────────────────────────────────────────

    fn check_function(
        &self,
        func: &Function,
        file_scope: &HashSet<String>,
        qt_props: &HashMap<String, String>,
        qt_signals: &HashMap<String, Vec<String>>,
        declared_signals: &[crate::types::Signal],
        child_id_map: &HashMap<String, ChildInfo>,
        errors: &mut Vec<AE>,
        context: Option<&str>,
    ) {
        // Sprawdź czy handler sygnału ma odpowiadający sygnał/property
        if func.is_signal_handler {
            let signal_name = handler_to_signal(&func.name);
            let prop_exists = qt_props.contains_key(&signal_name) || file_scope.contains(&signal_name);
            let signal_exists = qt_signals.contains_key(&format!("{signal_name}Changed"))
                || qt_signals.contains_key(&signal_name)
                || declared_signals.iter().any(|s| s.name == signal_name)
                // handler onXxxChanged → property xxx istnieje?
                || {
                    let maybe_prop = strip_changed_suffix(&signal_name);
                    maybe_prop.is_some_and(|p| qt_props.contains_key(p) || file_scope.contains(p))
                };

            if !prop_exists && !signal_exists {
                let mut e = AE::new(ErrorKind::UnknownSignalHandler {
                    handler: func.name.clone(),
                })
                .with_line(func.line);
                if let Some(c) = context {
                    e = e.with_context(c);
                }
                errors.push(e);
            }
        }

        // Zbuduj lokalny zasięg: parametry + wszystkie nazwy użyte w ciele funkcji
        // (parser nie rozróżnia deklaracji od użycia – traktujemy wszystkie nazwy
        //  obecne w ciele jako potencjalnie lokalne, Plan: nie sprawdzamy zakresu zmiennych)
        let mut local: HashSet<String> = func.parameters.iter().cloned().collect();
        for used in &func.used_names {
            if !file_scope.contains(&used.name) && !is_js_global(&used.name) {
                local.insert(used.name.clone());
            }
        }

        let _all_scope: HashSet<&str> = file_scope
            .iter()
            .map(String::as_str)
            .chain(func.parameters.iter().map(String::as_str))
            .chain(local.iter().map(String::as_str))
            .chain(JS_GLOBALS.iter().copied())
            .collect();

        // Zbuduj TYLKO file_scope + parameters + declared_locals + JS_GLOBALS (bez local!)
        let strict_scope: HashSet<&str> = file_scope
            .iter()
            .map(String::as_str)
            .chain(func.parameters.iter().map(String::as_str))
            .chain(func.declared_locals.iter().map(String::as_str))
            .chain(JS_GLOBALS.iter().copied())
            .collect();

        let mut seen_bases: HashSet<&str> = HashSet::new();
        let mut seen_cpp_members: HashSet<(String, String)> = HashSet::new();
        let mut seen_qml_members: HashSet<(String, String)> = HashSet::new();
        for used in &func.used_names {
            let name = used.name.as_str();
            // Use the per-name line if available, otherwise fall back to the function start line.
            let err_line = if used.line > 0 { used.line } else { func.line };

            if let Some(accessed) = &used.accessed_item {
                // Check C++ object member access if the object has known members.
                let key = (name.to_string(), accessed.clone());
                if seen_cpp_members.insert(key)
                    && let Some(members_opt) = self.ctx.cpp_object_members.get(name)
                    && let Some(members) = members_opt
                    && !members.contains(accessed.as_str())
                {
                    let mut e = AE::new(ErrorKind::UnknownCppMember {
                        object: name.to_string(),
                        member: accessed.clone(),
                    })
                    .with_line(err_line);
                    if let Some(c) = context {
                        e = e.with_context(c);
                    }
                    errors.push(e);
                }

                // Check member access on known QML children (non-C++ objects).
                // Fires when the child's type is known (Qt DB, user QML, or has declared properties).
                // Skip entirely when the name is a function parameter or a declared local variable —
                // their types are unknown (parameters are untyped; locals may be arrow-function params
                // or `let`/`const` bindings that shadow a child id).
                if !self.ctx.cpp_object_members.contains_key(name)
                    && !func.parameters.iter().any(|p| p == name)
                    && !func.declared_locals.iter().any(|l| l == name)
                    && let Some(child_info) = child_id_map.get(name)
                    && child_info.type_name != "Connections"
                    && seen_qml_members.insert((name.to_string(), accessed.clone()))
                {
                    let file_member_names = self.all_file_member_names(&child_info.type_name);
                    let qt_base = self.resolve_qt_type(&child_info.type_name);
                    if !file_member_names.is_empty()
                        || !child_info.properties.is_empty()
                        || self.db.has_type(&child_info.type_name)
                        || self.db.has_type(&qt_base)
                    {
                        let qt_child_props = self.db.all_properties(&qt_base);
                        let qt_child_sigs = self.db.all_signals(&qt_base);
                        let qt_child_methods = self.db.all_methods(&qt_base);
                        // For Loader content proxies, also allow Loader's own methods (e.g. setSource)
                        // and Loader's own properties (e.g. `item`, `status`, `source`).
                        let loader_methods = if child_info.is_loader_content {
                            self.db.all_methods("Loader")
                        } else {
                            HashMap::new()
                        };
                        let loader_own_props = if child_info.is_loader_content {
                            self.db.all_properties("Loader")
                        } else {
                            HashMap::new()
                        };
                        // Auto-generated xxxChanged signal for any known property.
                        let is_auto_signal = accessed.strip_suffix("Changed").is_some_and(|base| {
                            file_member_names.contains(base)
                                || qt_child_props.contains_key(base)
                                || child_info.properties.iter().any(|p| p.name == base)
                        });
                        let member_valid = file_member_names.contains(accessed.as_str())
                            || qt_child_props.contains_key(accessed.as_str())
                            || qt_child_sigs.contains_key(accessed.as_str())
                            || qt_child_methods.contains_key(accessed.as_str())
                            || loader_methods.contains_key(accessed.as_str())
                            || loader_own_props.contains_key(accessed.as_str())
                            || child_info.properties.iter().any(|p| p.name == accessed.as_str())
                            || child_info.function_names.iter().any(|f| f == accessed.as_str())
                            || is_auto_signal
                            // For `parent`, all Item properties are always valid because any
                            // visual element is Item-derived and anchor/size properties are universal.
                            || (name == "parent"
                                && self.db.all_properties("Item").contains_key(accessed.as_str()));
                        if !member_valid {
                            let mut e = AE::new(ErrorKind::UnknownQmlMember {
                                object: name.to_string(),
                                member: accessed.clone(),
                            })
                            .with_line(err_line);
                            if let Some(c) = context {
                                e = e.with_context(c);
                            }
                            errors.push(e);
                        }
                    }
                }
                // Also check that the base name itself is in scope
                // (e.g. `nnnonono_existent.state` → `nnnonono_existent` must be defined).
                if !seen_bases.contains(name) {
                    seen_bases.insert(name);
                    if !strict_scope.contains(name) && !self.db.has_type(name) {
                        let mut e = AE::new(ErrorKind::UndefinedName {
                            name: name.to_string(),
                            function: func.name.clone(),
                        })
                        .with_line(err_line);
                        if let Some(c) = context {
                            e = e.with_context(c);
                        }
                        errors.push(e);
                    }
                }
                continue;
            }

            if seen_bases.contains(name) {
                continue;
            }
            seen_bases.insert(name);

            if !strict_scope.contains(name) && !self.db.has_type(name) {
                let mut e = AE::new(ErrorKind::UndefinedName {
                    name: name.to_string(),
                    function: func.name.clone(),
                })
                .with_line(err_line);
                if let Some(c) = context {
                    e = e.with_context(c);
                }
                errors.push(e);
            }
        }

        // Sprawdź member assignments: obj.prop = value
        for assignment in &func.member_assignments {
            // C++ objects: skip child-element checks (member access was already
            // validated in the used_names loop above via accessed_item).
            if self.ctx.cpp_object_members.contains_key(&assignment.object) {
                continue;
            }

            // Function parameters are untyped JS values — skip member validation entirely.
            // Declared locals (arrow-function params, let/const/var) are also untyped — skip them.
            if func.parameters.iter().any(|p| p == &assignment.object)
                || func.declared_locals.iter().any(|l| l == &assignment.object)
            {
                continue;
            }

            let Some(child_info) = child_id_map.get(&assignment.object) else {
                // Nie znamy tego obiektu (może być JS local) — pomijamy
                continue;
            };

            // Connections objects are treated as opaque — their API is dynamic.
            if child_info.type_name == "Connections" {
                continue;
            }

            // Jeśli typ dziecka jest całkowicie nieznany (nie w Qt DB, nie ma zadeklarowanych
            // properties) — traktujemy jako opaque i zezwalamy na wszystkie operacje.
            let resolved_qt = self.resolve_qt_type(&child_info.type_name);
            if !self.db.has_type(&child_info.type_name) && !self.db.has_type(&resolved_qt) && child_info.properties.is_empty() {
                continue;
            }

            let child_qt_props = self.db.all_properties(&resolved_qt);
            let child_qt_methods = self.db.all_methods(&resolved_qt);
            let qt_type_str = child_qt_props.get(&assignment.member);
            let decl_prop = child_info.properties.iter().find(|p| p.name == assignment.member);
            // Also check properties and functions declared in the type's own QML file,
            // and functions declared inline in this child block (e.g. `function foo() {}`).
            let file_member_names = self.all_file_member_names(&child_info.type_name);
            let inline_func = child_info.function_names.iter().any(|f| f == &assignment.member);
            // Auto-generated xxxChanged signal counts as a valid assignment target.
            let is_auto_signal = assignment.member.strip_suffix("Changed").is_some_and(|base| {
                file_member_names.contains(base) || child_qt_props.contains_key(base)
            });

            if qt_type_str.is_none()
                && decl_prop.is_none()
                && !file_member_names.contains(assignment.member.as_str())
                && !inline_func
                && !child_qt_methods.contains_key(assignment.member.as_str())
                && !is_auto_signal
            {
                let mut e = AE::new(ErrorKind::UnknownMemberAccess {
                    object: assignment.object.clone(),
                    member: assignment.member.clone(),
                })
                .with_line(func.line);
                if let Some(c) = context {
                    e = e.with_context(c);
                }
                errors.push(e);
                continue;
            }

            // Sprawdź zgodność typu
            if matches!(assignment.value, PropertyValue::TooComplex | PropertyValue::Unset) {
                continue;
            }
            let expected_type = if let Some(decl) = decl_prop {
                decl.prop_type.clone()
            } else if let Some(qt_str) = qt_type_str {
                PropertyType::from_token(qt_str)
            } else {
                continue;
            };

            if matches!(expected_type, PropertyType::Var) {
                continue;
            }

            if let Some(mismatch) = Self::literal_type_mismatch(&expected_type, &assignment.value) {
                let mut e = AE::new(ErrorKind::MemberAssignmentTypeMismatch {
                    object: assignment.object.clone(),
                    member: assignment.member.clone(),
                    expected: prop_type_name(&expected_type),
                    assigned: mismatch,
                })
                .with_line(func.line);
                if let Some(c) = context {
                    e = e.with_context(c);
                }
                errors.push(e);
            }
        }
    }

    // ── sprawdzanie dzieci ────────────────────────────────────────────────

    /// Validate a `Connections { … }` child element.
    ///
    /// Checks:
    /// 1. `target:` is a known name in scope (only in project mode).
    /// 2. Signal handler names match the target's signals (when the target type is known).
    /// 3. Function body names are valid against the parent scope.
    fn check_connections_child(
        &self,
        child: &QmlChild,
        parent_scope: &HashSet<String>,
        errors: &mut Vec<AE>,
        file_id_map: &HashMap<String, ChildInfo>,
    ) {
        let ctx_str = child.id.as_deref().unwrap_or("Connections").to_string();

        // Extract target name and its source line.
        let target_entry = child.assignments.iter().find(|(k, _, _)| k == "target");
        let target_name = target_entry.map(|(_, v, _)| v.as_str());
        let target_line = target_entry.and_then(|(_, _, l)| if *l > 0 { Some(*l) } else { None });

        // ── 1. Validate target is in scope (project mode only) ─────────────────
        if let Some(target) = target_name {
            let target_known = parent_scope.contains(target)
                || self.ctx.cpp_object_members.contains_key(target)
                || is_js_global(target);

            // Only report in project mode (same gate as UnknownType).
            if !target_known && (!self.ctx.known_types.is_empty() || !self.ctx.cpp_globals.is_empty()) {
                let mut e = AE::new(ErrorKind::UnknownConnectionsTarget { name: target.to_string() });
                if let Some(ln) = target_line {
                    e = e.with_line(ln);
                }
                e = e.with_context(&ctx_str);
                errors.push(e);
            }
        }

        // ── 2. Build target's signals for handler name validation ──────────────
        let mut target_qt_props: HashMap<String, String> = HashMap::new();
        let mut target_qt_signals: HashMap<String, Vec<String>> = HashMap::new();

        let allow_any_handler = |funcs: &[crate::types::Function],
                                 props: &mut HashMap<String, String>,
                                 signals: &mut HashMap<String, Vec<String>>| {
            for func in funcs {
                if func.is_signal_handler {
                    let sname = handler_to_signal(&func.name);
                    signals.insert(sname.clone(), vec![]);
                    props.insert(sname, "var".to_string());
                }
            }
        };

        match target_name {
            Some(target) if self.ctx.cpp_object_members.contains_key(target) => {
                match self.ctx.cpp_object_members.get(target) {
                    Some(Some(members)) => {
                        // Known C++ object with explicit member list → validate handlers.
                        for m in members {
                            target_qt_signals.insert(m.clone(), vec![]);
                            target_qt_props.insert(m.clone(), "var".to_string());
                        }
                    }
                    _ => {
                        // Opaque C++ object → allow all declared handler names.
                        allow_any_handler(&child.functions, &mut target_qt_props, &mut target_qt_signals);
                    }
                }
            }
            Some(target) if file_id_map.contains_key(target) => {
                // QML child by id → use its Qt type's signals and properties.
                let child_info = &file_id_map[target];
                let qt_type = self.resolve_qt_type(&child_info.type_name);
                target_qt_props = self.db.all_properties(&qt_type);
                target_qt_signals = self.db.all_signals(&qt_type);
                // Include signals declared directly on this child instance.
                for s in &child_info.signal_names {
                    target_qt_signals.insert(s.clone(), vec![]);
                    target_qt_props.insert(s.clone(), "var".to_string());
                }
                // Also include signals/properties from the user-defined QML type chain
                // (e.g. NavigationExamPanel has `signal stopPressed` not in the Qt DB).
                let mut current = child_info.type_name.clone();
                let mut seen = HashSet::new();
                while seen.insert(current.clone()) {
                    if let Some((props, sigs)) = self.ctx.file_members.get(&current) {
                        for s in sigs {
                            target_qt_signals.insert(s.clone(), vec![]);
                        }
                        for p in props {
                            target_qt_props.entry(p.clone()).or_insert_with(|| "var".to_string());
                        }
                    }
                    match self.ctx.file_base_types.get(&current) {
                        Some(b) if self.ctx.file_members.contains_key(b.as_str()) => current = b.clone(),
                        _ => break,
                    }
                }
            }
            _ => {
                // Target unknown, in scope but non-id, or no target → allow all handlers.
                allow_any_handler(&child.functions, &mut target_qt_props, &mut target_qt_signals);
            }
        }

        // ── 3. Check each handler function ────────────────────────────────────
        let mut connections_scope = parent_scope.clone();
        connections_scope.insert("target".to_string());
        if let Some(id) = &child.id {
            connections_scope.insert(id.clone());
        }
        for prop in &child.properties {
            connections_scope.insert(prop.name.clone());
        }

        for func in &child.functions {
            self.check_function(
                func,
                &connections_scope,
                &target_qt_props,
                &target_qt_signals,
                &[], // no declared signals — target signals already in target_qt_signals
                file_id_map,
                errors,
                Some(&ctx_str),
            );
        }
    }

    fn check_child(
        &self,
        child: &QmlChild,
        parent_scope: &HashSet<String>,
        errors: &mut Vec<AE>,
        elem_path: &[String],
        file_id_map: &HashMap<String, ChildInfo>,
        import_aliases: &HashMap<String, bool>,
    ) {
        // Connections blocks get their own specialised check.
        if child.type_name == "Connections" {
            self.check_connections_child(child, parent_scope, errors, file_id_map);
            return;
        }

        let ctx = child.id.as_deref().unwrap_or(&child.type_name).to_string();

        // Build the element path for this child (used in --complex mode).
        let mut my_elem_path = elem_path.to_vec();
        my_elem_path.push(ctx.clone());

        // Resolve aliased module types, e.g. `QQC2.ApplicationWindow` (alias `QQC2`
        // from `import QtQuick.Controls as QQC2`):
        //   - Strip the alias prefix and look up the bare name in the Qt DB.
        //   - If found → use it as the effective type for full validation.
        //   - If NOT found (e.g. `Kirigami.Icon`) → opaque external type, skip silently.
        let (resolved_type_name, child_full_validation): (&str, bool) = if let Some(dot_pos) = child.type_name.find('.')
        {
            let prefix = &child.type_name[..dot_pos];
            if import_aliases.contains_key(prefix) {
                match self.resolve_aliased_base(&child.type_name, import_aliases) {
                    // name unchanged means type not in DB → opaque external type, skip silently
                    Some((name, _)) if name == child.type_name => return,
                    Some((name, full)) => (name, full),
                    None => return, // shouldn't happen (prefix is a known alias)
                }
            } else {
                (&child.type_name, true)
            }
        } else {
            (&child.type_name, true)
        };

        // Sprawdź czy typ jest znany — albo Qt, albo sparsowany plik QML.
        if !self.ctx.known_types.is_empty()
            && !self.db.has_type(resolved_type_name)
            && !self.ctx.known_types.contains(resolved_type_name)
        {
            errors.push(
                AE::new(ErrorKind::UnknownType {
                    type_name: child.type_name.clone(),
                })
                .with_line(child.line),
            );
            return;
        }

        // Dla znanych typów QML (pliki .qml) używamy ich typu bazowego do wyszukiwania Qt props.
        let effective_type = self.resolve_qt_type(resolved_type_name);
        let qt_props = self.db.all_properties(&effective_type);
        let qt_signals = self.db.all_signals(&effective_type);

        // Zbuduj zasięg dziecka = zasięg rodzica + własne property + własne id
        let mut child_scope = parent_scope.clone();
        if let Some(id) = &child.id {
            child_scope.insert(id.clone());
        }
        for p in &child.properties {
            child_scope.insert(p.name.clone());
            // Auto-generated <propName>Changed signal is also in scope.
            child_scope.insert(format!("{}Changed", p.name));
        }
        for name in qt_props.keys() {
            child_scope.insert(name.clone());
        }
        let qt_methods = self.db.all_methods(&effective_type);
        for name in qt_methods.keys() {
            child_scope.insert(name.clone());
        }
        // Qt signals are also callable (to emit them, e.g. `pressAndHold()` in `onDoubleClicked`).
        for name in qt_signals.keys() {
            child_scope.insert(name.clone());
        }
        // Add properties/signals from QML base type chain (e.g. Sub4 → SwitchWrapper → switchWrapperColor).
        for name in self.all_file_member_names(resolved_type_name) {
            child_scope.insert(name);
        }
        // Add the child's own declared functions so sibling handlers can call them.
        for f in &child.functions {
            child_scope.insert(f.name.clone());
        }

        let child_prop_types: HashMap<String, &PropertyType> = child
            .properties
            .iter()
            .map(|p| (p.name.clone(), &p.prop_type))
            .collect();

        // Track errors generated at this level so we can attach element_path in --complex mode.
        let level_start = errors.len();

        for prop in &child.properties {
            Self::check_property_decl(
                prop,
                &qt_props,
                &child_prop_types,
                &child_scope,
                self.db,
                errors,
                Some(&ctx),
            );
        }

        // C++ member validation for child property declaration expressions.
        for prop in &child.properties {
            if prop.raw_value_expr.is_empty() {
                continue;
            }
            let mut seen_cpp: HashSet<(String, String)> = HashSet::new();
            for (base_name, member_opt) in collect_dotted_accesses_from_expression(&prop.raw_value_expr) {
                let Some(accessed) = member_opt else { continue };
                let key = (base_name.clone(), accessed.clone());
                if seen_cpp.insert(key)
                    && let Some(members_opt) = self.ctx.cpp_object_members.get(&base_name)
                    && let Some(members) = members_opt
                    && !members.contains(accessed.as_str())
                {
                    errors.push(
                        AE::new(ErrorKind::UnknownCppMember {
                            object: base_name.clone(),
                            member: accessed,
                        })
                        .with_line(prop.line)
                        .with_context(&ctx),
                    );
                }
            }
        }

        for func in &child.functions {
            self.check_function(
                func,
                &child_scope,
                &qt_props,
                &qt_signals,
                &[],
                file_id_map, // file-wide id map: QML ids are file-scoped
                errors,
                Some(&ctx),
            );
        }

        // Check JS-block property value bodies (e.g. `text: { if (foo) ... }`).
        for func in &child.property_js_block_funcs {
            self.check_function(func, &child_scope, &qt_props, &qt_signals, &[], file_id_map, errors, Some(&ctx));
        }

        // Check inline property assignments against known props.
        let type_member_names = self.all_file_member_names(resolved_type_name);
        let mut seen_qml_assign: HashSet<(String, String)> = HashSet::new();
        for (name, value_expr, line) in &child.assignments {
            if child_full_validation {
                let prop_known = name.contains('.')
                    || qt_props.contains_key(name.as_str())
                    || child.properties.iter().any(|p| p.name == *name)
                    || type_member_names.contains(name.as_str());
                if !prop_known {
                    errors.push(
                        AE::new(ErrorKind::UnknownPropertyAssignment { name: name.clone() })
                            .with_line(*line)
                            .with_context(&ctx),
                    );
                }
            }
            if !is_inline_type_instantiation(value_expr) {
                let mut seen_names: HashSet<String> = HashSet::new();
                let mut seen_cpp: HashSet<(String, String)> = HashSet::new();
                for (base_name, member) in collect_dotted_accesses_from_expression(value_expr) {
                    // Validate C++ member access (base.member)
                    if let Some(accessed) = &member {
                        let key = (base_name.clone(), accessed.clone());
                        if seen_cpp.insert(key)
                            && let Some(members_opt) = self.ctx.cpp_object_members.get(&base_name)
                            && let Some(members) = members_opt
                            && !members.contains(accessed.as_str())
                        {
                            errors.push(
                                AE::new(ErrorKind::UnknownCppMember {
                                    object: base_name.clone(),
                                    member: accessed.clone(),
                                })
                                .with_line(*line)
                                .with_context(&ctx),
                            );
                        }
                    }

                    // Also validate QML child member access.
                    if let Some(accessed) = &member {
                        if !self.ctx.cpp_object_members.contains_key(&base_name) {
                            if let Some(child_info2) = file_id_map.get(&base_name) {
                                if child_info2.type_name != "Connections" {
                                    let child_member_names = self.all_file_member_names(&child_info2.type_name);
                                    let qt_base2 = self.resolve_qt_type(&child_info2.type_name);
                                    if !child_member_names.is_empty()
                                        || !child_info2.properties.is_empty()
                                        || self.db.has_type(&child_info2.type_name)
                                        || self.db.has_type(&qt_base2)
                                    {
                                        let qt_child_props2 = self.db.all_properties(&qt_base2);
                                        let qt_child_sigs2 = self.db.all_signals(&qt_base2);
                                        let qt_child_methods2 = self.db.all_methods(&qt_base2);
                                        let loader_methods2 = if child_info2.is_loader_content {
                                            self.db.all_methods("Loader")
                                        } else {
                                            HashMap::new()
                                        };
                                        let loader_own_props2 = if child_info2.is_loader_content {
                                            self.db.all_properties("Loader")
                                        } else {
                                            HashMap::new()
                                        };
                                        let is_auto_sig =
                                            accessed.strip_suffix("Changed").is_some_and(|base| {
                                                child_member_names.contains(base)
                                                    || qt_child_props2.contains_key(base)
                                                    || child_info2.properties.iter().any(|p| p.name == base)
                                            });
                                        let member_valid = child_member_names.contains(accessed.as_str())
                                            || qt_child_props2.contains_key(accessed.as_str())
                                            || qt_child_sigs2.contains_key(accessed.as_str())
                                            || qt_child_methods2.contains_key(accessed.as_str())
                                            || loader_methods2.contains_key(accessed.as_str())
                                            || loader_own_props2.contains_key(accessed.as_str())
                                            || child_info2.properties.iter().any(|p| p.name == accessed.as_str())
                                            || child_info2.function_names.iter().any(|f| f == accessed.as_str())
                                            || is_auto_sig
                                            || (base_name == "parent"
                                                && self.db.all_properties("Item").contains_key(accessed.as_str()));
                                        if !member_valid {
                                            let seen_key = (base_name.clone(), accessed.clone());
                                            if seen_qml_assign.insert(seen_key) {
                                                errors.push(
                                                    AE::new(ErrorKind::UnknownQmlMember {
                                                        object: base_name.clone(),
                                                        member: accessed.clone(),
                                                    })
                                                    .with_line(*line)
                                                    .with_context(&ctx),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !seen_names.insert(base_name.clone()) {
                        continue;
                    }
                    // Skip Qt types used as enum namespaces (e.g. Text.AlignLeft)
                    if self.db.has_type(&base_name) {
                        continue;
                    }
                    if !child_scope.contains(base_name.as_str()) && !is_js_global(&base_name) {
                        errors.push(
                            AE::new(ErrorKind::UndefinedPropertyAccess {
                                prop: name.clone(),
                                name: base_name.clone(),
                            })
                            .with_line(*line)
                            .with_context(&ctx),
                        );
                    }
                }
            }
        }

        // In --complex mode, stamp all errors generated at this level with the element path.
        if self.ctx.complex {
            for e in &mut errors[level_start..] {
                e.element_path = my_elem_path.clone();
            }
        }

        // For grandchildren, their QML `parent` is the current child element.
        // Exception: Repeater and Component don't create a visual parent context —
        // Repeater delegates have their runtime parent set to Repeater.parent,
        // and Component templates are instantiated elsewhere.
        // For these, pass through the outer parent unchanged.
        let grandchild_id_map = if matches!(child.type_name.as_str(), "Repeater" | "Component") {
            file_id_map.clone()
        } else {
            let mut m = file_id_map.clone();
            m.insert(
                "parent".to_string(),
                ChildInfo {
                    type_name: child.type_name.clone(),
                    properties: child.properties.clone(),
                    function_names: child
                        .functions
                        .iter()
                        .filter(|f| !f.is_signal_handler)
                        .map(|f| f.name.clone())
                        .chain(child.signals.iter().map(|s| s.name.clone()))
                        .collect(),
                    signal_names: child.signals.iter().map(|s| s.name.clone()).collect(),
                    is_loader_content: false,
                },
            );
            m
        };
        for grandchild in &child.children {
            self.check_child(
                grandchild,
                &child_scope,
                errors,
                &my_elem_path,
                &grandchild_id_map,
                import_aliases,
            );
        }
    }
}
