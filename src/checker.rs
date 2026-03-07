use std::collections::{HashMap, HashSet};

use crate::parser::collect_dotted_accesses_from_expression;
use crate::qt_types::QtTypeDb;
use crate::types::{FileItem, Function, Property, PropertyType, PropertyValue, QmlChild};

// ── typy błędów ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// `property int height` w Rectangle – height już istnieje w bazie Qt
    PropertyRedefinition { name: String, base_type: String },
    /// `property int foo: false` – zadeklarowany typ != typ przypisywanej wartości
    PropertyTypeMismatch {
        name: String,
        declared: String,
        assigned: String,
    },
    /// `property bool b: otherIntProp` – zadeklarowany typ != typ wskazanej property
    PropertyRefTypeMismatch {
        name: String,
        declared: String,
        ref_name: String,
        ref_type: String,
    },
    /// `Layout.notExist: true` – property nie istnieje w typie
    UnknownPropertyAssignment { name: String },
    /// `Layout.fillWidth: 21` – typ wartości nie zgadza się z oczekiwanym
    AssignmentTypeMismatch {
        name: String,
        expected: String,
        assigned: String,
    },
    /// `zzzz = x + y` wewnątrz funkcji – nazwa niezdefiniowana nigdzie w zasięgu
    UndefinedName { name: String, function: String },
    /// `function onStatussChanged()` – sygnał o tej nazwie nie istnieje
    UnknownSignalHandler { handler: String },
    /// `element.nonExisting = true` – property nie istnieje na elemencie
    UnknownMemberAccess { object: String, member: String },
    /// `element.boolProp = 0` – typ przypisanej wartości niezgodny z typem property
    MemberAssignmentTypeMismatch {
        object: String,
        member: String,
        expected: String,
        assigned: String,
    },
    /// `property bool foo: undeclared.bar` – nazwa w wyrażeniu property nie jest w zasięgu
    UndefinedPropertyAccess { prop: String, name: String },
    /// `Sub3 { }` – typ nie jest zdefiniowany nigdzie w projekcie ani w Qt
    UnknownType { type_name: String },
    /// `diskManager.nonExisting` — member not found in C++ object
    UnknownCppMember { object: String, member: String },
}

#[derive(Debug, Clone)]
pub struct AnalysisError {
    pub kind: ErrorKind,
    /// Opcjonalny kontekst (np. nazwa elementu dziecka)
    pub context: Option<String>,
    /// Source line number (1-based), if known.
    pub line: Option<usize>,
    /// Full element path within the file (populated in --complex mode).
    /// E.g. ["Row", "cancelButton"] means the error is inside the cancelButton
    /// child of a Row which is a direct child of the file root.
    pub element_path: Vec<String>,
}

impl AnalysisError {
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: None,
            line: None,
            element_path: Vec::new(),
        }
    }
    fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }
    fn with_line(mut self, line: usize) -> Self {
        if line > 0 {
            self.line = Some(line);
        }
        self
    }
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match &self.context {
            Some(c) => format!("[{c}] "),
            None => String::new(),
        };
        match &self.kind {
            ErrorKind::PropertyRedefinition { name, base_type } => write!(
                f,
                "{prefix}Property `{name}` redefines existing property from `{base_type}`"
            ),
            ErrorKind::PropertyTypeMismatch {
                name,
                declared,
                assigned,
            } => write!(
                f,
                "{prefix}Property `{name}` declared as `{declared}` but assigned `{assigned}` literal"
            ),
            ErrorKind::PropertyRefTypeMismatch {
                name,
                declared,
                ref_name,
                ref_type,
            } => write!(
                f,
                "{prefix}Property `{name}` declared as `{declared}` but assigned property `{ref_name}` of type `{ref_type}`"
            ),
            ErrorKind::UnknownPropertyAssignment { name } => {
                write!(f, "{prefix}Assignment to unknown property `{name}`")
            }
            ErrorKind::AssignmentTypeMismatch {
                name,
                expected,
                assigned,
            } => write!(f, "{prefix}Property `{name}` expects `{expected}` but got `{assigned}`"),
            ErrorKind::UndefinedName { name, function } => {
                write!(f, "{prefix}Undefined name `{name}` used in function `{function}`")
            }
            ErrorKind::UnknownSignalHandler { handler } => write!(
                f,
                "{prefix}Signal handler `{handler}` has no corresponding signal or property"
            ),
            ErrorKind::UnknownMemberAccess { object, member } => {
                write!(f, "{prefix}Assignment to unknown property `{member}` on `{object}`")
            }
            ErrorKind::MemberAssignmentTypeMismatch {
                object,
                member,
                expected,
                assigned,
            } => write!(
                f,
                "{prefix}`{object}.{member}` expects `{expected}` but got `{assigned}`"
            ),
            ErrorKind::UndefinedPropertyAccess { prop, name } => {
                write!(f, "{prefix}Undefined name `{name}` used in property `{prop}`")
            }
            ErrorKind::UnknownType { type_name } => write!(f, "{prefix}Unknown type `{type_name}`"),
            ErrorKind::UnknownCppMember { object, member } => {
                write!(f, "{prefix}Unknown member `{member}` on C++ object `{object}`")
            }
        }
    }
}

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
struct ChildInfo {
    type_name: String,
    properties: Vec<Property>,
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
                },
            );
        }
        // Rekurencyjnie — id zagnieżdżonych dzieci też są widoczne
        map.extend(build_child_id_map(&child.children));
    }
    map
}

impl Checker<'_> {
    // ── rozwiązywanie łańcucha typów bazowych ────────────────────────────

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
        // property root
        for p in &file.properties {
            scope.insert(p.name.clone());
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
        let resolved_base = self.resolve_qt_type(&file.base_type);
        for (name, _) in self.db.all_properties(&resolved_base) {
            scope.insert(name);
        }
        // Properties/signals from user-defined QML base type chain
        // e.g. Global extends WindowBase → windowBusy from WindowBase is in scope
        {
            let mut current = file.base_type.clone();
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
            scope.extend(parent_scope.iter().cloned());
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

    fn check_root(&self, file: &FileItem, file_scope: &HashSet<String>, errors: &mut Vec<AnalysisError>) {
        let effective_base = self.resolve_qt_type(&file.base_type);
        let qt_props = self.db.all_properties(&effective_base);
        let qt_signals = self.db.all_signals(&effective_base);

        // Budujemy mapę property pliku (name → PropertyType) do sprawdzania referencji
        let file_prop_types: HashMap<String, &PropertyType> =
            file.properties.iter().map(|p| (p.name.clone(), &p.prop_type)).collect();

        // Mapa id dzieci → info o dziecku (do sprawdzania member access w funkcjach)
        let mut child_id_map = build_child_id_map(&file.children);
        // Syntetyczne dzieci z konfiguracji (new_child)
        if let Some(child_types) = self.ctx.extra_children.get(&file.name) {
            for child_type in child_types {
                let id = type_name_to_id(child_type);
                child_id_map.entry(id).or_insert_with(|| ChildInfo {
                    type_name: child_type.clone(),
                    properties: vec![],
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
                    });
                }
            }
        }

        // 1. Sprawdź deklaracje property
        for prop in &file.properties {
            Self::check_property_decl(prop, &qt_props, &file_prop_types, file_scope, self.db, errors, None);
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

        // 3. Sprawdź root-level inline assignments (e.g. `width: expr`, `invalidProp: val`).
        let type_member_names = self.all_file_member_names(&file.name);
        for (name, value_expr, line) in &file.assignments {
            let prop_known = qt_props.contains_key(name.as_str())
                || file.properties.iter().any(|p| p.name == *name)
                || type_member_names.contains(name.as_str());
            if !prop_known {
                errors.push(
                    AnalysisError::new(ErrorKind::UnknownPropertyAssignment { name: name.clone() }).with_line(*line),
                );
            }
            // Check names in the value expression against scope.
            // Skip complex values that start with `{` (standalone JS object literal).
            if !value_expr.trim_start().starts_with('{') {
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
                                AnalysisError::new(ErrorKind::UnknownCppMember {
                                    object: base_name.clone(),
                                    member: accessed.clone(),
                                })
                                .with_line(*line),
                            );
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
                            AnalysisError::new(ErrorKind::UndefinedPropertyAccess {
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
        for child in &file.children {
            self.check_child(child, file_scope, errors, &[]);
        }
    }

    // ── sprawdzanie property ──────────────────────────────────────────────

    fn check_property_decl(
        prop: &Property,
        qt_props: &HashMap<String, String>,
        file_prop_types: &HashMap<String, &PropertyType>,
        scope: &HashSet<String>,
        db: &QtTypeDb,
        errors: &mut Vec<AnalysisError>,
        context: Option<&str>,
    ) {
        // 1. Redefinicja property z klasy bazowej
        if qt_props.contains_key(&prop.name) {
            let base = "base type".to_string();
            let mut e = AnalysisError::new(ErrorKind::PropertyRedefinition {
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
            let mut e = AnalysisError::new(ErrorKind::PropertyTypeMismatch {
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
                let mut e = AnalysisError::new(ErrorKind::PropertyRefTypeMismatch {
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
                    let mut e = AnalysisError::new(ErrorKind::UndefinedPropertyAccess {
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
        errors: &mut Vec<AnalysisError>,
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
                let mut e = AnalysisError::new(ErrorKind::UnknownSignalHandler {
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

        // Nazwy które nie istnieją nigdzie = błąd.
        // Ponieważ powyżej dodaliśmy wszystkie used_names do local,
        // jedynymi undefined są te, które nie zostały zgłoszone przez żadne
        // znane źródło. Strategia: zgłaszamy tylko te, które nie mają żadnego
        // „partnera" – tzn. nie mają accessed_item (bo samo `zzzz` bez żadnego
        // przypisania lokalnego jest błędem).
        //
        // Uproszczenie z Planu: sprawdzamy czy nazwa istnieje w którymkolwiek zasięgu.
        // Skoro local zawiera wszystkie used_names, wszystko będzie w all_scope.
        // Musimy zatem inaczej: nie dodawać do local nazw które NIE są w file_scope.
        // Zamiast tego: tylko nazwy których nie ma w file_scope ORAZ nie są wyłącznie
        // po lewej stronie = (przypisanie), co parser też zbiera.
        //
        // Rzeczywiste podejście: collect_function_body_names zbiera WSZYSTKIE identyfikatory
        // z ciała. Nie wiemy które są deklaracjami. Plan mówi "nie sprawdzamy zakresu zmiennych".
        // Dlatego: sprawdzamy tylko te nazwy, które są w `used_names` BEZ accessed_item
        // ORAZ nie ma ich w file_scope. Jeśli nazwa ma accessed_item (foo.bar) – ignorujemy,
        // bo używana jako obiekt (var/local). Bez accessed_item i bez file_scope → undefined.

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
        for used in &func.used_names {
            let name = used.name.as_str();

            if let Some(accessed) = &used.accessed_item {
                // Check C++ object member access if the object has known members.
                let key = (name.to_string(), accessed.clone());
                if seen_cpp_members.insert(key)
                    && let Some(members_opt) = self.ctx.cpp_object_members.get(name)
                    && let Some(members) = members_opt
                    && !members.contains(accessed.as_str())
                {
                    let mut e = AnalysisError::new(ErrorKind::UnknownCppMember {
                        object: name.to_string(),
                        member: accessed.clone(),
                    })
                    .with_line(func.line);
                    if let Some(c) = context {
                        e = e.with_context(c);
                    }
                    errors.push(e);
                }
                // Also check that the base name itself is in scope
                // (e.g. `nnnonono_existent.state` → `nnnonono_existent` must be defined).
                if !seen_bases.contains(name) {
                    seen_bases.insert(name);
                    if !strict_scope.contains(name) {
                        let mut e = AnalysisError::new(ErrorKind::UndefinedName {
                            name: name.to_string(),
                            function: func.name.clone(),
                        })
                        .with_line(func.line);
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

            if !strict_scope.contains(name) {
                let mut e = AnalysisError::new(ErrorKind::UndefinedName {
                    name: name.to_string(),
                    function: func.name.clone(),
                })
                .with_line(func.line);
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
            if !self.db.has_type(&child_info.type_name) && child_info.properties.is_empty() {
                continue;
            }

            let child_qt_props = self.db.all_properties(&child_info.type_name);
            let qt_type_str = child_qt_props.get(&assignment.member);
            let decl_prop = child_info.properties.iter().find(|p| p.name == assignment.member);

            if qt_type_str.is_none() && decl_prop.is_none() {
                let mut e = AnalysisError::new(ErrorKind::UnknownMemberAccess {
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
                let mut e = AnalysisError::new(ErrorKind::MemberAssignmentTypeMismatch {
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

    fn check_child(
        &self,
        child: &QmlChild,
        parent_scope: &HashSet<String>,
        errors: &mut Vec<AnalysisError>,
        elem_path: &[String],
    ) {
        let ctx = child.id.as_deref().unwrap_or(&child.type_name).to_string();

        // Build the element path for this child (used in --complex mode).
        let mut my_elem_path = elem_path.to_vec();
        my_elem_path.push(ctx.clone());

        // Sprawdź czy typ jest znany — albo Qt, albo sparsowany plik QML.
        // Sprawdzamy tylko gdy known_types nie jest puste (tzn. mamy pełen kontekst projektu).
        // Puste known_types oznacza tryb izolowany (testy jednostkowe) — pomijamy sprawdzanie.
        if !self.ctx.known_types.is_empty()
            && !self.db.has_type(&child.type_name)
            && !self.ctx.known_types.contains(&child.type_name)
        {
            errors.push(
                AnalysisError::new(ErrorKind::UnknownType {
                    type_name: child.type_name.clone(),
                })
                .with_line(child.line),
            );
            return;
        }

        // Dla znanych typów QML (pliki .qml) używamy ich typu bazowego do wyszukiwania Qt props.
        // Np. Sub2 extends Switch → qt_props = Switch's properties (includes `checked`).
        // Musimy iść przez cały łańcuch typów bazowych aż do Qt type (TextButton → GenericButton → RoundButton).
        let effective_type = self.resolve_qt_type(&child.type_name);
        let qt_props = self.db.all_properties(&effective_type);
        let qt_signals = self.db.all_signals(&effective_type);

        // Zbuduj zasięg dziecka = zasięg rodzica + własne property + własne id
        let mut child_scope = parent_scope.clone();
        if let Some(id) = &child.id {
            child_scope.insert(id.clone());
        }
        for p in &child.properties {
            child_scope.insert(p.name.clone());
        }
        for name in qt_props.keys() {
            child_scope.insert(name.clone());
        }
        // Add properties/signals from QML base type chain (e.g. Sub4 → SwitchWrapper → switchWrapperColor).
        for name in self.all_file_member_names(&child.type_name) {
            child_scope.insert(name);
        }

        let child_prop_types: HashMap<String, &PropertyType> = child
            .properties
            .iter()
            .map(|p| (p.name.clone(), &p.prop_type))
            .collect();
        let grandchild_id_map = build_child_id_map(&child.children);

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

        for func in &child.functions {
            self.check_function(
                func,
                &child_scope,
                &qt_props,
                &qt_signals,
                &[],
                &grandchild_id_map,
                errors,
                Some(&ctx),
            );
        }

        // Check inline property assignments (e.g. `non_existtttttttend: 2`) against known props,
        // and check names used in the value expression against scope.
        let type_member_names = self.all_file_member_names(&child.type_name);
        for (name, value_expr, line) in &child.assignments {
            let prop_known = qt_props.contains_key(name.as_str())
                || child.properties.iter().any(|p| p.name == *name)
                || type_member_names.contains(name.as_str());
            if !prop_known {
                errors.push(
                    AnalysisError::new(ErrorKind::UnknownPropertyAssignment { name: name.clone() })
                        .with_line(*line)
                        .with_context(&ctx),
                );
            }
            // Check names used in the value expression against scope.
            // Skip standalone object literals `{ … }` — they're not expressions with names.
            if !value_expr.trim_start().starts_with('{') {
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
                                AnalysisError::new(ErrorKind::UnknownCppMember {
                                    object: base_name.clone(),
                                    member: accessed.clone(),
                                })
                                .with_line(*line)
                                .with_context(&ctx),
                            );
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
                            AnalysisError::new(ErrorKind::UndefinedPropertyAccess {
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

        for grandchild in &child.children {
            self.check_child(grandchild, &child_scope, errors, &my_elem_path);
        }
    }
}

// ── pomocnicze ────────────────────────────────────────────────────────────

// `onWidthChanged` → `widthChanged` lub `width`
fn handler_to_signal(handler: &str) -> String {
    let body = &handler[2..]; // strip `on`
    let mut chars = body.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// `widthChanged` → Some("width")
fn strip_changed_suffix(s: &str) -> Option<&str> {
    s.strip_suffix("Changed")
}

/// `NewExaminationScreen` → `newExaminationScreen`
fn type_name_to_id(type_name: &str) -> String {
    let mut chars = type_name.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn prop_type_name(t: &PropertyType) -> String {
    match t {
        PropertyType::Int => "int".into(),
        PropertyType::Bool => "bool".into(),
        PropertyType::String => "string".into(),
        PropertyType::Var => "var".into(),
        PropertyType::Double => "double".into(),
        PropertyType::Real => "real".into(),
        PropertyType::Url => "url".into(),
        PropertyType::Color => "color".into(),
        PropertyType::List => "list".into(),
        PropertyType::Custom(s) => s.clone(),
    }
}

fn types_compatible(a: &PropertyType, b: &PropertyType) -> bool {
    if matches!(a, PropertyType::Var) || matches!(b, PropertyType::Var) {
        return true;
    }
    // real i double są kompatybilne
    let numeric = |t: &PropertyType| matches!(t, PropertyType::Double | PropertyType::Real);
    if numeric(a) && numeric(b) {
        return true;
    }
    a == b
}

fn is_js_global(name: &str) -> bool {
    JS_GLOBALS.contains(&name)
}

/// QML-specyficzne zmienne dostępne w kontekście delegatów modeli.
const QML_DELEGATE_GLOBALS: &[&str] = &[
    "model",     // obiekt modelu w delegacie (model.field)
    "modelData", // dane elementu w prostym delegacie
    "index",     // indeks elementu w delegacie
];

const JS_GLOBALS: &[&str] = &[
    "console",
    "Math",
    "JSON",
    "parseInt",
    "parseFloat",
    "qsTr",
    "Qt",
    "undefined",
    "null",
    "true",
    "false",
    "NaN",
    "Infinity",
    "String",
    "Number",
    "Boolean",
    "Array",
    "Object",
    "Date",
    "RegExp",
    "Error",
    "Promise",
    "Symbol",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "x",
    "y",
    "z", // często lokalne w lambdach
];
