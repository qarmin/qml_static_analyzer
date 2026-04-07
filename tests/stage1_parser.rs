//! Integration tests for Stage 1: QML parser.
//!
//! Each sub-test corresponds to a directory under `test_resources/`.
//! Tests use `pretty_assertions` so that diff output is human-readable.
//!
//! Snapshot behaviour
//! ------------------
//! * First run  → `test_resources/<dir>/snapshot.ron` is **created**.
//! * Later runs → the file is **compared**; mismatch = test failure.
//! * To accept a change: delete the `.ron` file and re-run.

use std::fs;
use std::path::Path;

use pretty_assertions::assert_eq;
use qml_static_analyzer::parser::parse_file;
use qml_static_analyzer::snapshot::{assert_inline_snapshot, assert_snapshot};
use qml_static_analyzer::types::{
    FileItem, Function, FunctionUsedName, Property, PropertyType, PropertyValue, QmlChild, Signal, SignalParameter,
};

// ─── helper ─────────────────────────────────────────────────────────────────

fn read_qml(dir: &str, file: &str) -> (String, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_resources")
        .join(dir)
        .join(file);
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    let name = file.trim_end_matches(".qml").to_string();
    (name, source)
}

// ─── test: simple file ───────────────────────────────────────────────────────

#[test]
fn test_simple_file() {
    let (name, source) = read_qml("test_simple_file", "SimpleItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    let expected = FileItem {
        name: "SimpleItem".to_string(),
        base_type: "Item".to_string(),
        id: Some("root".to_string()),
        imports: vec!["import QtQuick".to_string()],
        signals: vec![],
        properties: vec![],
        functions: vec![],
        children: vec![],
        assignments: vec![],
        property_js_block_funcs: vec![],
    };

    assert_eq!(result, expected);
    assert_snapshot("test_simple_file", &result);
}

// ─── test: properties ────────────────────────────────────────────────────────

#[test]
fn test_properties() {
    let (name, source) = read_qml("test_properties", "PropertiesItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        name: String,
        base_type: String,
        properties: Vec<Property>,
    }

    assert_eq!(
        Result {
            name: result.name.clone(),
            base_type: result.base_type.clone(),
            properties: result.properties.clone(),
        },
        Result {
            name: "PropertiesItem".to_string(),
            base_type: "Rectangle".to_string(),
            properties: vec![
                Property {
                    name: "count".to_string(),
                    prop_type: PropertyType::Int,
                    value: PropertyValue::Int(42),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "visible2".to_string(),
                    prop_type: PropertyType::Bool,
                    value: PropertyValue::Bool(true),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "label".to_string(),
                    prop_type: PropertyType::String,
                    value: PropertyValue::String("hello".to_string()),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "anyValue".to_string(),
                    prop_type: PropertyType::Var,
                    value: PropertyValue::Int(99),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "ratio".to_string(),
                    prop_type: PropertyType::Double,
                    value: PropertyValue::Double(3.13),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
            ],
        }
    );
    assert_snapshot("test_properties", &result);
}

// ─── test: signals ───────────────────────────────────────────────────────────

#[test]
fn test_signals() {
    let (name, source) = read_qml("test_signals", "SignalsItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        name: String,
        base_type: String,
        signals: Vec<Signal>,
    }

    assert_eq!(
        Result {
            name: result.name.clone(),
            base_type: result.base_type.clone(),
            signals: result.signals.clone(),
        },
        Result {
            name: "SignalsItem".to_string(),
            base_type: "Item".to_string(),
            signals: vec![
                Signal {
                    name: "clicked".to_string(),
                    parameters: vec![],
                },
                Signal {
                    name: "valueChanged".to_string(),
                    parameters: vec![SignalParameter {
                        param_type: "int".to_string(),
                        param_name: "newValue".to_string(),
                    }],
                },
                Signal {
                    name: "textUpdated".to_string(),
                    parameters: vec![
                        SignalParameter {
                            param_type: "string".to_string(),
                            param_name: "text".to_string(),
                        },
                        SignalParameter {
                            param_type: "bool".to_string(),
                            param_name: "force".to_string(),
                        },
                    ],
                },
            ],
        }
    );
    assert_snapshot("test_signals", &result);
}

// ─── test: functions ─────────────────────────────────────────────────────────

#[test]
fn test_functions() {
    let (name, source) = read_qml("test_functions", "FunctionsItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        functions: Vec<Function>,
    }

    assert_eq!(
        Result {
            functions: result.functions.clone()
        },
        Result {
            functions: vec![
                Function {
                    name: "increment".to_string(),
                    is_signal_handler: false,
                    parameters: vec![],
                    ..result.functions.iter().find(|f| f.name == "increment").unwrap().clone()
                },
                Function {
                    name: "reset".to_string(),
                    is_signal_handler: false,
                    parameters: vec![],
                    ..result.functions.iter().find(|f| f.name == "reset").unwrap().clone()
                },
                Function {
                    name: "computeSum".to_string(),
                    is_signal_handler: false,
                    parameters: vec!["a".to_string(), "b".to_string()],
                    ..result
                        .functions
                        .iter()
                        .find(|f| f.name == "computeSum")
                        .unwrap()
                        .clone()
                },
                Function {
                    name: "onCounterChanged".to_string(),
                    is_signal_handler: true,
                    parameters: vec![],
                    ..result
                        .functions
                        .iter()
                        .find(|f| f.name == "onCounterChanged")
                        .unwrap()
                        .clone()
                },
            ],
        }
    );

    // Verify used_names in `increment`: counter is referenced
    let increment_used: Vec<&FunctionUsedName> = result
        .functions
        .iter()
        .find(|f| f.name == "increment")
        .unwrap()
        .used_names
        .iter()
        .collect();
    assert_eq!(
        increment_used.iter().any(|u| u.name == "counter"),
        true,
        "increment should reference `counter`, got: {:?}",
        increment_used
    );

    // Verify computeSum references `result`
    let compute_used: Vec<&FunctionUsedName> = result
        .functions
        .iter()
        .find(|f| f.name == "computeSum")
        .unwrap()
        .used_names
        .iter()
        .collect();
    assert_eq!(
        compute_used.iter().any(|u| u.name == "result"),
        true,
        "computeSum should reference `result`, got: {:?}",
        compute_used
    );

    assert_snapshot("test_functions", &result);
}

// ─── test: children ──────────────────────────────────────────────────────────

#[test]
fn test_children() {
    let (name, source) = read_qml("test_children", "ChildrenItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        name: String,
        base_type: String,
        properties: Vec<Property>,
        children: Vec<QmlChild>,
    }

    assert_eq!(
        Result {
            name: result.name.clone(),
            base_type: result.base_type.clone(),
            properties: result.properties.clone(),
            children: result.children.clone(),
        },
        Result {
            name: "ChildrenItem".to_string(),
            base_type: "Item".to_string(),
            properties: vec![Property {
                name: "value".to_string(),
                prop_type: PropertyType::Int,
                value: PropertyValue::Int(10),
                accessed_properties: vec![],
                is_simple_ref: false,
                raw_value_expr: String::new(),
                line: 0,
            }],
            children: vec![
                QmlChild {
                    type_name: "Rectangle".to_string(),
                    id: Some("childRect".to_string()),
                    signals: vec![],
                    properties: vec![Property {
                        name: "label".to_string(),
                        prop_type: PropertyType::String,
                        value: PropertyValue::String("child".to_string()),
                        accessed_properties: vec![],
                        is_simple_ref: false,
                        raw_value_expr: String::new(),
                        line: 0,
                    }],
                    functions: vec![Function {
                        name: "doSomething".to_string(),
                        is_signal_handler: false,
                        parameters: vec![],
                        ..result.children[0].functions[0].clone()
                    }],
                    children: vec![],
                    assignments: vec![],
                    property_js_block_funcs: vec![],
                    line: 0,
                    is_loader_content: false,
                },
                QmlChild {
                    type_name: "Item".to_string(),
                    id: Some("innerItem".to_string()),
                    signals: vec![],
                    properties: vec![Property {
                        name: "active".to_string(),
                        prop_type: PropertyType::Bool,
                        value: PropertyValue::Bool(false),
                        accessed_properties: vec![],
                        is_simple_ref: false,
                        raw_value_expr: String::new(),
                        line: 0,
                    }],
                    functions: vec![],
                    line: 0,
                    assignments: vec![],
                    property_js_block_funcs: vec![],
                    is_loader_content: false,
                    children: vec![QmlChild {
                        type_name: "Rectangle".to_string(),
                        id: Some("deepRect".to_string()),
                        signals: vec![],
                        properties: vec![Property {
                            name: "depth".to_string(),
                            prop_type: PropertyType::Int,
                            value: PropertyValue::Int(2),
                            accessed_properties: vec![],
                            is_simple_ref: false,
                            raw_value_expr: String::new(),
                            line: 0,
                        }],
                        functions: vec![],
                        children: vec![],
                        assignments: vec![],
                        property_js_block_funcs: vec![],
                        line: 0,
                        is_loader_content: false,
                    }],
                },
            ],
        }
    );
    assert_snapshot("test_children", &result);
}

// ─── test: complex file ──────────────────────────────────────────────────────

#[test]
fn test_complex_file() {
    let (name, source) = read_qml("test_complex", "ComplexItem.qml");
    let result = parse_file(&name, &source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        name: String,
        base_type: String,
        id: Option<String>,
        imports: Vec<String>,
        signals: Vec<Signal>,
        properties: Vec<Property>,
        functions: Vec<Function>,
        children: Vec<QmlChild>,
    }

    assert_eq!(
        Result {
            name: result.name.clone(),
            base_type: result.base_type.clone(),
            id: result.id.clone(),
            imports: result.imports.clone(),
            signals: result.signals.clone(),
            properties: result.properties.clone(),
            functions: result.functions.clone(),
            children: result.children.clone(),
        },
        Result {
            name: "ComplexItem".to_string(),
            base_type: "Window".to_string(),
            id: Some("mainWindow".to_string()),
            imports: result.imports.clone(), // checked separately below
            signals: vec![Signal {
                name: "exportDriveChanged".to_string(),
                parameters: vec![],
            }],
            properties: vec![
                Property {
                    name: "processInBackground".to_string(),
                    prop_type: PropertyType::Bool,
                    value: PropertyValue::Bool(false),
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "intVal".to_string(),
                    prop_type: PropertyType::Int,
                    value: PropertyValue::Unset,
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
                Property {
                    name: "anyVar".to_string(),
                    prop_type: PropertyType::Var,
                    value: PropertyValue::Unset,
                    accessed_properties: vec![],
                    is_simple_ref: false,
                    raw_value_expr: String::new(),
                    line: 0,
                },
            ],
            functions: result.functions.clone(), // checked separately below
            children: result.children.clone(),   // checked separately below
        }
    );

    assert_eq!(result.imports.len(), 2);
    assert!(result.imports[0].contains("QtQuick"));
    assert!(result.imports[1].contains("Controls"));

    assert_eq!(result.functions.len(), 2);
    let on_width = result.functions.iter().find(|f| f.name == "onWidthChanged").unwrap();
    assert_eq!(
        (
            on_width.is_signal_handler,
            on_width.used_names.iter().any(|u| u.name == "intVal"),
            on_width
                .used_names
                .iter()
                .any(|u| u.name == "anyVar" && u.accessed_item.as_deref() == Some("expression"))
        ),
        (true, true, true),
        "onWidthChanged checks failed, used_names: {:?}",
        on_width.used_names
    );
    let on_export = result
        .functions
        .iter()
        .find(|f| f.name == "onExportDriveChanged")
        .unwrap();
    assert_eq!(on_export.is_signal_handler, true);

    assert_eq!(result.children.len(), 1);
    let child = &result.children[0];
    assert_eq!(
        (
            child.type_name.as_str(),
            child.id.as_deref(),
            child.functions.len(),
            child.functions[0].name.as_str(),
            child.functions[0].is_signal_handler
        ),
        ("Item", Some("internalElement"), 1, "internalFunction", false)
    );

    assert_snapshot("test_complex", &result);
}

// ─── test: property unset value ──────────────────────────────────────────────

#[test]
fn test_property_unset_value() {
    let source = "
Item {
    property int counter
    property bool flag
    property var data
}
";
    let result = parse_file("Test", source).expect("parse should succeed");

    assert_eq!(
        result.properties,
        vec![
            Property {
                name: "counter".to_string(),
                prop_type: PropertyType::Int,
                value: PropertyValue::Unset,
                accessed_properties: vec![],
                is_simple_ref: false,
                raw_value_expr: String::new(),
                line: 0,
            },
            Property {
                name: "flag".to_string(),
                prop_type: PropertyType::Bool,
                value: PropertyValue::Unset,
                accessed_properties: vec![],
                is_simple_ref: false,
                raw_value_expr: String::new(),
                line: 0,
            },
            Property {
                name: "data".to_string(),
                prop_type: PropertyType::Var,
                value: PropertyValue::Unset,
                accessed_properties: vec![],
                is_simple_ref: false,
                raw_value_expr: String::new(),
                line: 0,
            },
        ]
    );
    assert_inline_snapshot("property_unset_value", &result);
}

// ─── test: property with complex expression references accessed_properties ───

#[test]
fn test_property_accessed_properties() {
    let source = "
Item {
    property var combined: item2.something + item3.somethingElse
}
";
    let result = parse_file("Test", source).expect("parse should succeed");

    assert_eq!(result.properties.len(), 1);
    let prop = &result.properties[0];

    assert_eq!(
        (prop.value.clone(), {
            let mut ap = prop.accessed_properties.clone();
            ap.sort();
            ap
        }),
        (
            PropertyValue::TooComplex,
            vec!["item2".to_string(), "item3".to_string()]
        )
    );
    assert_inline_snapshot("property_accessed_properties", &result);
}

// ─── test: signal handler as function ────────────────────────────────────────

#[test]
fn test_signal_handler_function_is_flagged() {
    let source = "
Rectangle {
    function onWidthChanged() {
        let x = 1
    }
    function doWork() {
        let y = 2
    }
}
";
    let result = parse_file("Test", source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct FuncFlags {
        name: String,
        is_signal_handler: bool,
    }

    assert_eq!(
        result
            .functions
            .iter()
            .map(|f| FuncFlags {
                name: f.name.clone(),
                is_signal_handler: f.is_signal_handler
            })
            .collect::<Vec<_>>(),
        vec![
            FuncFlags {
                name: "onWidthChanged".to_string(),
                is_signal_handler: true
            },
            FuncFlags {
                name: "doWork".to_string(),
                is_signal_handler: false
            },
        ]
    );
}

// ─── test: inline signal handler block ───────────────────────────────────────

#[test]
fn test_inline_signal_handler_block() {
    let source = r#"
Rectangle {
    onHeightChanged: {
        console.log("Height changed")
    }
}
"#;
    let result = parse_file("Test", source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct FuncMeta {
        len: usize,
        name: String,
        is_signal_handler: bool,
        parameters: Vec<String>,
    }

    assert_eq!(
        FuncMeta {
            len: result.functions.len(),
            name: result.functions[0].name.clone(),
            is_signal_handler: result.functions[0].is_signal_handler,
            parameters: result.functions[0].parameters.clone(),
        },
        FuncMeta {
            len: 1,
            name: "onHeightChanged".to_string(),
            is_signal_handler: true,
            parameters: vec![],
        }
    );
}

// ─── test: no imports ────────────────────────────────────────────────────────

#[test]
fn test_no_imports() {
    let source = "
Item {
    id: minimal
}
";
    let result = parse_file("Minimal", source).expect("parse should succeed");

    #[derive(Debug, PartialEq)]
    struct Result {
        imports: Vec<String>,
        name: String,
        base_type: String,
        id: Option<String>,
    }

    assert_eq!(
        Result {
            imports: result.imports.clone(),
            name: result.name.clone(),
            base_type: result.base_type.clone(),
            id: result.id,
        },
        Result {
            imports: vec![],
            name: "Minimal".to_string(),
            base_type: "Item".to_string(),
            id: Some("minimal".to_string()),
        }
    );
}
