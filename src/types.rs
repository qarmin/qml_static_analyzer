use serde::{Deserialize, Serialize};

/// The type of a QML property (e.g. int, bool, string, var, …)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyType {
    Int,
    Bool,
    String,
    Var,
    Double,
    Real,
    Url,
    Color,
    List,
    Custom(String),
}

impl PropertyType {
    /// Parse a QML type keyword into a `PropertyType`.
    pub fn from_token(s: &str) -> Self {
        match s {
            "int" => Self::Int,
            "bool" => Self::Bool,
            "string" => Self::String,
            "var" => Self::Var,
            "double" => Self::Double,
            "real" => Self::Real,
            "url" => Self::Url,
            "color" => Self::Color,
            "list" => Self::List,
            other => Self::Custom(other.to_string()),
        }
    }
}

impl std::str::FromStr for PropertyType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_token(s))
    }
}

/// A literal or simple value of a property.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Int(i64),
    Bool(bool),
    String(String),
    Double(f64),
    Null,
    /// The value is an expression too complex to evaluate statically.
    TooComplex,
    /// No value provided (e.g. `property int foo`).
    Unset,
}

/// Describes a QML property declaration, e.g. `property int foo: 42`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub prop_type: PropertyType,
    pub value: PropertyValue,
    /// Names of other identifiers accessed while evaluating the initial value,
    /// e.g. `property var item: item2.something + item3.somethingElse`
    /// → accessed_properties = ["item2", "item3"]
    pub accessed_properties: Vec<String>,
    /// True when the value expression is exactly a single identifier with no operators,
    /// e.g. `property bool b: other` — used to enable cross-property type checking.
    /// False for complex expressions like `!other` or `a && b`.
    #[serde(skip, default)]
    pub is_simple_ref: bool,
    /// Source line number (1-based). Not serialized — used for error reporting only.
    #[serde(skip)]
    pub line: usize,
}

impl PartialEq for Property {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.prop_type == other.prop_type
            && self.value == other.value
            && self.accessed_properties == other.accessed_properties
        // `line` and `is_simple_ref` intentionally excluded — not persisted in snapshots
    }
}

/// A name/access pair used inside a function body, e.g. `item.something`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionUsedName {
    /// The base identifier, e.g. `item` in `item.something`
    pub name: String,
    /// The member being accessed, e.g. `something` in `item.something`
    pub accessed_item: Option<String>,
    /// Source line number (1-based). Not serialized — used for error reporting only.
    #[serde(skip, default)]
    pub line: usize,
}

impl PartialEq for FunctionUsedName {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.accessed_item == other.accessed_item
        // `line` intentionally excluded — it is not persisted in snapshots
    }
}

impl Eq for FunctionUsedName {}

/// A simple assignment to a child's property inside a function body,
/// e.g. `element.property = value` where value is a literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberAssignment {
    pub object: String,
    pub member: String,
    pub value: PropertyValue,
}

/// Describes a function or a signal handler, e.g. `function foo(a, b) { … }`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Function {
    pub name: String,
    /// `true` if the name starts with `on` (signal handler convention)
    pub is_signal_handler: bool,
    pub parameters: Vec<String>,
    /// All names (and optional member accesses) referenced inside the body
    pub used_names: Vec<FunctionUsedName>,
    /// Variables declared with `let`/`const`/`var` in the function body
    #[serde(default)]
    pub declared_locals: Vec<String>,
    /// Simple member assignments found in the function body, e.g. `elem.prop = value`
    #[serde(default)]
    pub member_assignments: Vec<MemberAssignment>,
    /// Source line number (1-based). Not serialized — used for error reporting only.
    #[serde(skip)]
    pub line: usize,
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.is_signal_handler == other.is_signal_handler
            && self.parameters == other.parameters
            && self.used_names == other.used_names
            && self.declared_locals == other.declared_locals
            && self.member_assignments == other.member_assignments
        // `line` intentionally excluded — it is not persisted in snapshots
    }
}

/// A child element declared inline, e.g. `Rectangle { … }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QmlChild {
    /// The type name of the element, e.g. `Rectangle`, `Item`
    pub type_name: String,
    /// Optional `id:` value
    pub id: Option<String>,
    /// Extra properties declared inside this child
    pub properties: Vec<Property>,
    /// Functions / signal handlers declared inside this child
    pub functions: Vec<Function>,
    /// Nested children
    pub children: Vec<Self>,
    /// Plain (non-dotted) inline property assignments, e.g. `color: "red"` or `non_exist: 2`.
    /// Tuple: (key, value_expr, line). Not serialized.
    #[serde(skip)]
    pub assignments: Vec<(String, String, usize)>,
    /// Source line number (1-based). Not serialized — used for error reporting only.
    #[serde(skip)]
    pub line: usize,
}

impl PartialEq for QmlChild {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name
            && self.id == other.id
            && self.properties == other.properties
            && self.functions == other.functions
            && self.children == other.children
        // `line` intentionally excluded — not persisted in snapshots
    }
}

/// Top-level representation of a single `.qml` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileItem {
    /// Derived from the filename without the `.qml` extension, e.g. `MyType`
    pub name: String,
    /// The type the file inherits from, e.g. `Item`, `Rectangle`
    pub base_type: String,
    /// Optional root-level `id:`
    pub id: Option<String>,
    /// All imports (module or file)
    pub imports: Vec<String>,
    /// Signal declarations at the top level
    pub signals: Vec<Signal>,
    /// Properties declared at the top level
    pub properties: Vec<Property>,
    /// Functions / signal handlers declared at the top level
    pub functions: Vec<Function>,
    /// Children of the root element
    pub children: Vec<QmlChild>,
    /// Plain (non-dotted) inline property assignments at the root level,
    /// e.g. `width: 400` or `invalidProp: true`. Tuple: (key, value_expr, line).
    /// Not serialized — used only during semantic checking.
    #[serde(skip)]
    pub assignments: Vec<(String, String, usize)>,
}

/// An explicit `signal` declaration, e.g. `signal clicked()` or
/// `signal valueChanged(int newValue)`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub name: String,
    pub parameters: Vec<SignalParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalParameter {
    pub param_type: String,
    pub param_name: String,
}
