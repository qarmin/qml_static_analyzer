//! Integration tests for multi-file QML parsing.
//!
//! Each test points at a directory under `test_resources/` that contains
//! several `.qml` files.  The entire directory is parsed with
//! `parse_directory` and the resulting `Vec<FileItem>` is verified both
//! through explicit assertions and through a RON snapshot.
//!
//! Snapshot behaviour – same as stage1:
//! * First run  -> `test_resources/<dir>/snapshot.ron` is **created**.
//! * Later runs -> file is **compared**; mismatch = test failure.
//! * To accept a change: delete the `.ron` file and re-run.
use std::path::Path;

use pretty_assertions::assert_eq;
use qml_static_analyzer::parser::parse_directory;
use qml_static_analyzer::snapshot::assert_dir_snapshot;
use qml_static_analyzer::types::{FileItem, PropertyType, PropertyValue};
// ─── helpers ─────────────────────────────────────────────────────────────────

fn parse_dir(subdir: &str) -> Vec<FileItem> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_resources")
        .join(subdir);
    parse_directory(&path).unwrap_or_else(|e| panic!("parse_directory failed for `{subdir}`: {e}"))
}

fn find_file<'a>(items: &'a [FileItem], name: &str) -> &'a FileItem {
    items
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("FileItem `{name}` not found in parsed results"))
}

// ─── test: basic multi-file directory ────────────────────────────────────────

#[test]
fn test_multi_basic() {
    let items = parse_dir("test_multi_basic");

    // Wrap all per-file expectations into one struct so there is a single assert.
    #[derive(Debug, PartialEq)]
    struct FileResult {
        name: String,
        base_type: String,
        id: Option<String>,
        import_count: usize,
        signal_names: Vec<String>,
        function_names: Vec<String>,
        all_functions_not_signal_handlers: bool,
        property_names: Vec<String>,
        label_value: PropertyValue,
        enabled_value: PropertyValue,
        click_count_value: PropertyValue,
    }

    #[derive(Debug, PartialEq)]
    struct PanelResult {
        name: String,
        base_type: String,
        id: Option<String>,
        signal_names: Vec<String>,
        signal0_param_name: String,
        function_names: Vec<String>,
        child_type: String,
        child_id: Option<String>,
    }

    #[derive(Debug, PartialEq)]
    struct SbResult {
        name: String,
        base_type: String,
        property_names: Vec<String>,
        message_value: PropertyValue,
        busy_value: PropertyValue,
    }

    #[derive(Debug, PartialEq)]
    struct AllResult {
        count: usize,
        names: Vec<String>,
        btn: FileResult,
        panel: PanelResult,
        sb: SbResult,
    }

    let btn = find_file(&items, "BaseButton");
    let panel = find_file(&items, "Panel");
    let sb = find_file(&items, "StatusBar");

    assert_eq!(
        AllResult {
            count: items.len(),
            names: items.iter().map(|f| f.name.clone()).collect(),
            btn: FileResult {
                name: btn.name.clone(),
                base_type: btn.base_type.clone(),
                id: btn.id.clone(),
                import_count: btn.imports.len(),
                signal_names: btn.signals.iter().map(|s| s.name.clone()).collect(),
                function_names: btn.functions.iter().map(|f| f.name.clone()).collect(),
                all_functions_not_signal_handlers: btn.functions.iter().all(|f| !f.is_signal_handler),
                property_names: btn.properties.iter().map(|p| p.name.clone()).collect(),
                label_value: btn.properties.iter().find(|p| p.name == "label").unwrap().value.clone(),
                enabled_value: btn
                    .properties
                    .iter()
                    .find(|p| p.name == "enabled")
                    .unwrap()
                    .value
                    .clone(),
                click_count_value: btn
                    .properties
                    .iter()
                    .find(|p| p.name == "clickCount")
                    .unwrap()
                    .value
                    .clone(),
            },
            panel: PanelResult {
                name: panel.name.clone(),
                base_type: panel.base_type.clone(),
                id: panel.id.clone(),
                signal_names: panel.signals.iter().map(|s| s.name.clone()).collect(),
                signal0_param_name: panel.signals[0].parameters[0].param_name.clone(),
                function_names: panel.functions.iter().map(|f| f.name.clone()).collect(),
                child_type: panel.children[0].type_name.clone(),
                child_id: panel.children[0].id.clone(),
            },
            sb: SbResult {
                name: sb.name.clone(),
                base_type: sb.base_type.clone(),
                property_names: sb.properties.iter().map(|p| p.name.clone()).collect(),
                message_value: sb
                    .properties
                    .iter()
                    .find(|p| p.name == "message")
                    .unwrap()
                    .value
                    .clone(),
                busy_value: sb.properties.iter().find(|p| p.name == "busy").unwrap().value.clone(),
            },
        },
        AllResult {
            count: 3,
            names: vec!["BaseButton".to_string(), "Panel".to_string(), "StatusBar".to_string()],
            btn: FileResult {
                name: "BaseButton".to_string(),
                base_type: "Rectangle".to_string(),
                id: Some("root".to_string()),
                import_count: 2,
                signal_names: vec!["clicked".to_string(), "longPressed".to_string()],
                function_names: vec!["reset".to_string(), "handlePress".to_string()],
                all_functions_not_signal_handlers: true,
                property_names: vec!["label".to_string(), "enabled".to_string(), "clickCount".to_string()],
                label_value: PropertyValue::String("Click me".to_string()),
                enabled_value: PropertyValue::Bool(true),
                click_count_value: PropertyValue::Int(0),
            },
            panel: PanelResult {
                name: "Panel".to_string(),
                base_type: "Item".to_string(),
                id: Some("container".to_string()),
                signal_names: vec!["titleChanged".to_string()],
                signal0_param_name: "newTitle".to_string(),
                function_names: vec!["setTitle".to_string()],
                child_type: "Rectangle".to_string(),
                child_id: Some("background".to_string()),
            },
            sb: SbResult {
                name: "StatusBar".to_string(),
                base_type: "Item".to_string(),
                property_names: vec!["message".to_string(), "busy".to_string()],
                message_value: PropertyValue::String("Ready".to_string()),
                busy_value: PropertyValue::Bool(false),
            },
        }
    );

    assert_dir_snapshot("test_multi_basic", &items);
}

// ─── test: inheritance chain ──────────────────────────────────────────────────

#[test]
fn test_multi_inheritance() {
    let items = parse_dir("test_multi_inheritance");

    #[derive(Debug, PartialEq)]
    struct ModelResult {
        name: String,
        base_type: String,
        id: Option<String>,
        property_names: Vec<String>,
        signal_names: Vec<String>,
        function_names: Vec<String>,
    }

    #[derive(Debug, PartialEq)]
    struct ExtResult {
        model: ModelResult,
        category_type: PropertyType,
        ratio_type: PropertyType,
        multiply_refs_value: bool,
        multiply_refs_ratio: bool,
    }

    #[derive(Debug, PartialEq)]
    struct SpecResult {
        model: ModelResult,
        lock_refs_locked: bool,
        lock_refs_priority: bool,
    }

    #[derive(Debug, PartialEq)]
    struct AllResult {
        count: usize,
        names: Vec<String>,
        base: ModelResult,
        ext: ExtResult,
        spec: SpecResult,
    }

    let base = find_file(&items, "BaseModel");
    let ext = find_file(&items, "ExtendedModel");
    let spec = find_file(&items, "SpecializedModel");
    let multiply = ext.functions.iter().find(|f| f.name == "multiply").unwrap();
    let lock = spec.functions.iter().find(|f| f.name == "lock").unwrap();

    assert_eq!(
        AllResult {
            count: items.len(),
            names: items.iter().map(|f| f.name.clone()).collect(),
            base: ModelResult {
                name: base.name.clone(),
                base_type: base.base_type.clone(),
                id: base.id.clone(),
                property_names: base.properties.iter().map(|p| p.name.clone()).collect(),
                signal_names: base.signals.iter().map(|s| s.name.clone()).collect(),
                function_names: base.functions.iter().map(|f| f.name.clone()).collect(),
            },
            ext: ExtResult {
                model: ModelResult {
                    name: ext.name.clone(),
                    base_type: ext.base_type.clone(),
                    id: ext.id.clone(),
                    property_names: ext.properties.iter().map(|p| p.name.clone()).collect(),
                    signal_names: ext.signals.iter().map(|s| s.name.clone()).collect(),
                    function_names: ext.functions.iter().map(|f| f.name.clone()).collect(),
                },
                category_type: ext
                    .properties
                    .iter()
                    .find(|p| p.name == "category")
                    .unwrap()
                    .prop_type
                    .clone(),
                ratio_type: ext
                    .properties
                    .iter()
                    .find(|p| p.name == "ratio")
                    .unwrap()
                    .prop_type
                    .clone(),
                multiply_refs_value: multiply.used_names.iter().any(|u| u.name == "value"),
                multiply_refs_ratio: multiply.used_names.iter().any(|u| u.name == "ratio"),
            },
            spec: SpecResult {
                model: ModelResult {
                    name: spec.name.clone(),
                    base_type: spec.base_type.clone(),
                    id: spec.id.clone(),
                    property_names: spec.properties.iter().map(|p| p.name.clone()).collect(),
                    signal_names: spec.signals.iter().map(|s| s.name.clone()).collect(),
                    function_names: spec.functions.iter().map(|f| f.name.clone()).collect(),
                },
                lock_refs_locked: lock.used_names.iter().any(|u| u.name == "locked"),
                lock_refs_priority: lock.used_names.iter().any(|u| u.name == "priority"),
            },
        },
        AllResult {
            count: 3,
            names: vec![
                "BaseModel".to_string(),
                "ExtendedModel".to_string(),
                "SpecializedModel".to_string()
            ],
            base: ModelResult {
                name: "BaseModel".to_string(),
                base_type: "Item".to_string(),
                id: Some("root".to_string()),
                property_names: vec!["value".to_string(), "name".to_string(), "active".to_string()],
                signal_names: vec!["valueChanged".to_string()],
                function_names: vec!["increment".to_string(), "reset".to_string()],
            },
            ext: ExtResult {
                model: ModelResult {
                    name: "ExtendedModel".to_string(),
                    base_type: "BaseModel".to_string(),
                    id: Some("extended".to_string()),
                    property_names: vec!["category".to_string(), "ratio".to_string(), "extra".to_string()],
                    signal_names: vec!["categoryChanged".to_string()],
                    function_names: vec!["setCategory".to_string(), "multiply".to_string()],
                },
                category_type: PropertyType::String,
                ratio_type: PropertyType::Double,
                multiply_refs_value: true,
                multiply_refs_ratio: true,
            },
            spec: SpecResult {
                model: ModelResult {
                    name: "SpecializedModel".to_string(),
                    base_type: "ExtendedModel".to_string(),
                    id: Some("specialized".to_string()),
                    property_names: vec!["tag".to_string(), "priority".to_string(), "locked".to_string()],
                    signal_names: vec!["locked".to_string()],
                    function_names: vec!["lock".to_string(), "unlock".to_string(), "applyTag".to_string()],
                },
                lock_refs_locked: true,
                lock_refs_priority: true,
            },
        }
    );

    assert_dir_snapshot("test_multi_inheritance", &items);
}

// ─── test: cross-file property references ────────────────────────────────────

#[test]
fn test_multi_references() {
    let items = parse_dir("test_multi_references");

    #[derive(Debug, PartialEq)]
    struct AllResult {
        count: usize,
        names: Vec<String>,
        ds_base_type: String,
        ds_property_count: usize,
        ds_signal_names: Vec<String>,
        lv_base_type: String,
        lv_model_value: PropertyValue,
        lv_model_accessed: Vec<String>,
        lv_selected_value: PropertyValue,
        lv_selected_accessed: Vec<String>,
        lv_loading_value: PropertyValue,
        lv_loading_accessed: Vec<String>,
        lv_refresh_refs_datasource: bool,
        lv_refresh_no_bare_load: bool,
        mv_base_type: String,
        mv_child_types: Vec<String>,
        mv_child_ids: Vec<Option<String>>,
        mv_item_count_refs_datasource: bool,
    }

    let ds = find_file(&items, "DataSource");
    let lv = find_file(&items, "ListView");
    let mv = find_file(&items, "MainView");
    let refresh = lv.functions.iter().find(|f| f.name == "refresh").unwrap();
    let item_count = mv.properties.iter().find(|p| p.name == "itemCount").unwrap();

    assert_eq!(
        AllResult {
            count: items.len(),
            names: items.iter().map(|f| f.name.clone()).collect(),
            ds_base_type: ds.base_type.clone(),
            ds_property_count: ds.properties.len(),
            ds_signal_names: ds.signals.iter().map(|s| s.name.clone()).collect(),
            lv_base_type: lv.base_type.clone(),
            lv_model_value: lv.properties.iter().find(|p| p.name == "model").unwrap().value.clone(),
            lv_model_accessed: lv
                .properties
                .iter()
                .find(|p| p.name == "model")
                .unwrap()
                .accessed_properties
                .clone(),
            lv_selected_value: lv
                .properties
                .iter()
                .find(|p| p.name == "selectedItem")
                .unwrap()
                .value
                .clone(),
            lv_selected_accessed: lv
                .properties
                .iter()
                .find(|p| p.name == "selectedItem")
                .unwrap()
                .accessed_properties
                .clone(),
            lv_loading_value: lv
                .properties
                .iter()
                .find(|p| p.name == "isLoading")
                .unwrap()
                .value
                .clone(),
            lv_loading_accessed: lv
                .properties
                .iter()
                .find(|p| p.name == "isLoading")
                .unwrap()
                .accessed_properties
                .clone(),
            lv_refresh_refs_datasource: refresh.used_names.iter().any(|u| u.name == "dataSource"),
            lv_refresh_no_bare_load: !refresh.used_names.iter().any(|u| u.name == "load"),
            mv_base_type: mv.base_type.clone(),
            mv_child_types: mv.children.iter().map(|c| c.type_name.clone()).collect(),
            mv_child_ids: mv.children.iter().map(|c| c.id.clone()).collect(),
            mv_item_count_refs_datasource: item_count.accessed_properties.contains(&"dataSource".to_string()),
        },
        AllResult {
            count: 3,
            names: vec!["DataSource".to_string(), "ListView".to_string(), "MainView".to_string()],
            ds_base_type: "Item".to_string(),
            ds_property_count: 3,
            ds_signal_names: vec!["dataReady".to_string(), "errorOccurred".to_string()],
            lv_base_type: "Item".to_string(),
            lv_model_value: PropertyValue::TooComplex,
            lv_model_accessed: vec!["dataSource".to_string()],
            lv_selected_value: PropertyValue::TooComplex,
            lv_selected_accessed: vec!["dataSource".to_string()],
            lv_loading_value: PropertyValue::TooComplex,
            lv_loading_accessed: vec!["dataSource".to_string()],
            lv_refresh_refs_datasource: true,
            lv_refresh_no_bare_load: true,
            mv_base_type: "Item".to_string(),
            mv_child_types: vec!["DataSource".to_string(), "ListView".to_string()],
            mv_child_ids: vec![Some("dataSource".to_string()), Some("listView".to_string())],
            mv_item_count_refs_datasource: true,
        }
    );

    assert_dir_snapshot("test_multi_references", &items);
}

// ─── test: nested custom-type children ───────────────────────────────────────

#[test]
fn test_multi_nested() {
    let items = parse_dir("test_multi_nested");

    #[derive(Debug, PartialEq)]
    struct AllResult {
        count: usize,
        names: Vec<String>,
        // Card
        card_base_type: String,
        card_property_names: Vec<String>,
        card_signal_names: Vec<String>,
        card_function_names: Vec<String>,
        // CardList
        cl_base_type: String,
        cl_property_count: usize,
        cl_signal_names: Vec<String>,
        cl_function_count: usize,
        cl_child_type: String,
        cl_child_id: Option<String>,
        cl_child_property_names: Vec<String>,
        cl_empty_value: PropertyValue,
        cl_empty_refs_card_count: bool,
        // Dashboard
        dash_base_type: String,
        dash_id: Option<String>,
        dash_property_count: usize,
        dash_total_cards_value: PropertyValue,
        dash_total_cards_accessed: Vec<String>,
        dash_child_type: String,
        dash_child_id: Option<String>,
        dash_child_property_names: Vec<String>,
        dash_child_children_types: Vec<String>,
        dash_child_children_ids: Vec<Option<String>>,
        dash_child_children_property_names: Vec<Vec<String>>,
    }

    let card = find_file(&items, "Card");
    let card_list = find_file(&items, "CardList");
    let dashboard = find_file(&items, "Dashboard");
    let empty_prop = card_list.properties.iter().find(|p| p.name == "empty").unwrap();
    let total_cards = dashboard.properties.iter().find(|p| p.name == "totalCards").unwrap();
    let child_list = &dashboard.children[0];

    assert_eq!(
        AllResult {
            count: items.len(),
            names: items.iter().map(|f| f.name.clone()).collect(),
            card_base_type: card.base_type.clone(),
            card_property_names: card.properties.iter().map(|p| p.name.clone()).collect(),
            card_signal_names: card.signals.iter().map(|s| s.name.clone()).collect(),
            card_function_names: card.functions.iter().map(|f| f.name.clone()).collect(),
            cl_base_type: card_list.base_type.clone(),
            cl_property_count: card_list.properties.len(),
            cl_signal_names: card_list.signals.iter().map(|s| s.name.clone()).collect(),
            cl_function_count: card_list.functions.len(),
            cl_child_type: card_list.children[0].type_name.clone(),
            cl_child_id: card_list.children[0].id.clone(),
            cl_child_property_names: card_list.children[0]
                .properties
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            cl_empty_value: empty_prop.value.clone(),
            cl_empty_refs_card_count: empty_prop.accessed_properties.contains(&"cardCount".to_string()),
            dash_base_type: dashboard.base_type.clone(),
            dash_id: dashboard.id.clone(),
            dash_property_count: dashboard.properties.len(),
            dash_total_cards_value: total_cards.value.clone(),
            dash_total_cards_accessed: total_cards.accessed_properties.clone(),
            dash_child_type: child_list.type_name.clone(),
            dash_child_id: child_list.id.clone(),
            dash_child_property_names: child_list.properties.iter().map(|p| p.name.clone()).collect(),
            dash_child_children_types: child_list.children.iter().map(|c| c.type_name.clone()).collect(),
            dash_child_children_ids: child_list.children.iter().map(|c| c.id.clone()).collect(),
            dash_child_children_property_names: child_list
                .children
                .iter()
                .map(|c| c.properties.iter().map(|p| p.name.clone()).collect())
                .collect(),
        },
        AllResult {
            count: 3,
            names: vec!["Card".to_string(), "CardList".to_string(), "Dashboard".to_string()],
            card_base_type: "Rectangle".to_string(),
            card_property_names: vec![
                "cardTitle".to_string(),
                "cardBody".to_string(),
                "backgroundColor".to_string(),
                "elevated".to_string()
            ],
            card_signal_names: vec!["cardClicked".to_string()],
            card_function_names: vec!["setContent".to_string()],
            cl_base_type: "Item".to_string(),
            cl_property_count: 3,
            cl_signal_names: vec!["cardAdded".to_string(), "cardRemoved".to_string()],
            cl_function_count: 2,
            cl_child_type: "Card".to_string(),
            cl_child_id: Some("placeholderCard".to_string()),
            cl_child_property_names: vec!["placeholder".to_string()],
            cl_empty_value: PropertyValue::TooComplex,
            cl_empty_refs_card_count: true,
            dash_base_type: "Item".to_string(),
            dash_id: Some("dashboard".to_string()),
            dash_property_count: 3,
            dash_total_cards_value: PropertyValue::TooComplex,
            dash_total_cards_accessed: vec!["cardList".to_string()],
            dash_child_type: "CardList".to_string(),
            dash_child_id: Some("cardList".to_string()),
            dash_child_property_names: vec!["owner".to_string()],
            dash_child_children_types: vec!["Card".to_string(), "Card".to_string()],
            dash_child_children_ids: vec![Some("welcomeCard".to_string()), Some("infoCard".to_string())],
            dash_child_children_property_names: vec![vec!["greeting".to_string()], vec!["info".to_string()]],
        }
    );

    assert_dir_snapshot("test_multi_nested", &items);
}
