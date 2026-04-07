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
    /// `Connections { target: unknownObj }` — target not in QML scope
    UnknownConnectionsTarget { name: String },
    /// `someChild.nonExistentMethod()` — member not declared on known QML child type
    UnknownQmlMember { object: String, member: String },
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
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: None,
            line: None,
            element_path: Vec::new(),
        }
    }
    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }
    pub fn with_line(mut self, line: usize) -> Self {
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
            ErrorKind::UnknownConnectionsTarget { name } => {
                write!(f, "{prefix}Connections target `{name}` is not defined")
            }
            ErrorKind::UnknownQmlMember { object, member } => {
                write!(f, "{prefix}Unknown member `{member}` on `{object}`")
            }
        }
    }
}
