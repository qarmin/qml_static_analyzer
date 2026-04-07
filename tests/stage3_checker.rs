//! Integration tests for the semantic checker (Stage 3).
//!
//! Each test parses an inline QML snippet, runs `check_file`, and asserts
//! exactly which errors (if any) are produced.

use qml_static_analyzer::checker::{CheckContext, ErrorKind, check_file};
use qml_static_analyzer::parser::parse_file;
use qml_static_analyzer::qt_types::{QtTypeDb, load_from_json_file};

// ─── helper ──────────────────────────────────────────────────────────────────

/// Load the Qt type database.
///
/// Requires the `QT_TYPES_JSON` env var to be set to a Qt types JSON file.
/// Use `just test` to run tests with the correct environment.
fn load_db() -> QtTypeDb {
    let path = std::env::var("QT_TYPES_JSON").expect(
        "\n\nQT_TYPES_JSON env var is not set.\n\
         Please run tests via `just test` instead of `cargo test` directly.\n"
    );
    load_from_json_file(std::path::Path::new(&path)).expect("failed to load Qt types from QT_TYPES_JSON")
}

fn check(source: &str) -> Vec<ErrorKind> {
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = CheckContext::empty();
    check_file(&file, &db, &ctx).into_iter().map(|e| e.kind).collect()
}

fn has_error(errors: &[ErrorKind], pred: impl Fn(&ErrorKind) -> bool) -> bool {
    errors.iter().any(pred)
}

// ─── Loader: single static source ────────────────────────────────────────────

/// A Loader with a static `source:` pointing at a `.qml` file should produce
/// a synthetic child whose type_name equals the filename stem.
#[test]
fn test_loader_single_source_parsed_as_child() {
    let source = r#"
Item {
    Loader {
        id: contentLoader
        anchors.fill: parent
        source: "qrc:/components/window/WelcomeContent.qml"
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    assert_eq!(file.children.len(), 1, "Loader should produce one synthetic child");
    let child = &file.children[0];
    assert_eq!(child.type_name, "WelcomeContent");
    assert_eq!(child.id.as_deref(), Some("contentLoader"));
}

// ─── Loader: ternary source → two children ───────────────────────────────────

/// When the `source:` expression is a ternary with two `.qml` references, both
/// possible types should appear as children.
#[test]
fn test_loader_ternary_source_parsed_as_two_children() {
    let source = r#"
Item {
    Loader {
        id: globalsLoader
        anchors.fill: parent
        source: fullyInitialized ? "qrc:/components/window/Globals.qml" : ""
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    assert_eq!(
        file.children.len(),
        1,
        "Loader with one non-empty .qml should produce one child"
    );
    let child = &file.children[0];
    assert_eq!(child.type_name, "Globals");
    assert_eq!(child.id.as_deref(), Some("globalsLoader"));
}

/// Ternary with two non-empty .qml paths → two children, both with the loader id.
#[test]
fn test_loader_ternary_two_qml_sources() {
    let source = r#"
Item {
    Loader {
        id: myLoader
        source: flag ? "qrc:/components/A.qml" : "qrc:/components/B.qml"
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    assert_eq!(
        file.children.len(),
        2,
        "Ternary with two .qml refs should produce two children"
    );
    let types: Vec<&str> = file.children.iter().map(|c| c.type_name.as_str()).collect();
    assert!(types.contains(&"A"), "child A expected");
    assert!(types.contains(&"B"), "child B expected");
    // Both children carry the loader id
    for child in &file.children {
        assert_eq!(child.id.as_deref(), Some("myLoader"));
    }
}

/// Multiple Loaders → children from each.
#[test]
fn test_multiple_loaders_produce_children() {
    let source = r#"
Item {
    Loader {
        id: contentLoader
        source: "qrc:/components/window/WelcomeContent.qml"
    }
    Loader {
        id: globalsLoader
        source: "qrc:/components/window/Globals.qml"
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    assert_eq!(file.children.len(), 2);
    assert_eq!(file.children[0].type_name, "WelcomeContent");
    assert_eq!(file.children[0].id.as_deref(), Some("contentLoader"));
    assert_eq!(file.children[1].type_name, "Globals");
    assert_eq!(file.children[1].id.as_deref(), Some("globalsLoader"));
}

// ─── Connections: parsed and validated ───────────────────────────────────────

/// Connections block is now parsed: it appears as a child and its handler
/// function bodies are checked for undefined names.
#[test]
fn test_connections_block_ignored() {
    let source = "
Item {
    property bool hardwareKeyboardAvailable: false

    Connections {
        target: settingsManager
        function onKeyboardLanguageChanged() {
            VirtualKeyboardSettings.locale = settingsManager.keyboardLanguage
        }
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    // Connections is now a proper child element
    assert_eq!(file.children.len(), 1, "Connections block should appear as a child");
    assert_eq!(file.children[0].type_name, "Connections");
    // Handler function is extracted
    assert_eq!(file.children[0].functions.len(), 1);
    // Root-level functions are still 0
    assert_eq!(file.functions.len(), 0, "Connections functions must not leak to root");

    // VirtualKeyboardSettings is not in scope → UndefinedName is expected
    let errors = check(source);
    assert!(
        errors.iter().any(|e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "VirtualKeyboardSettings"
        )),
        "Expected UndefinedName for VirtualKeyboardSettings, got: {errors:?}"
    );
}

/// Connections block appears as a child and its handler bodies are validated.
/// References inside handler functions are checked against the parent scope.
#[test]
fn test_connections_inside_nested_item() {
    let source = "
Rectangle {
    Item {
        Connections {
            target: someTarget
            function onFooChanged() {
                undefinedThing = 1
            }
        }
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    assert_eq!(file.children.len(), 1, "only the Item child expected");
    let item_child = &file.children[0];
    assert_eq!(item_child.type_name, "Item");
    // Connections is now parsed as a child of Item
    assert_eq!(item_child.children.len(), 1, "Connections should appear as a child of Item");
    assert_eq!(item_child.children[0].type_name, "Connections");

    // undefinedThing is not in scope — it must be reported
    let errors = check(source);
    assert!(
        errors.iter().any(|e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "undefinedThing"
        )),
        "Expected UndefinedName for undefinedThing, got: {errors:?}"
    );
}

// ─── Checker: child id in scope (showKeyboardButton case) ────────────────────

/// A child element's `id` must be in scope for functions of the parent element.
/// `showKeyboardButton.visible = hardwareKeyboardAvailable` should produce NO error
/// because `showKeyboardButton` is the id of a child and `hardwareKeyboardAvailable`
/// is a declared property.
#[test]
fn test_child_id_is_in_scope_for_parent_functions() {
    let source = "
Item {
    property bool hardwareKeyboardAvailable: false

    ShowKeyboardButton {
        id: showKeyboardButton
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
    }

    function updateVisibility() {
        showKeyboardButton.visible = hardwareKeyboardAvailable
    }
}
";
    let errors = check(source);
    // showKeyboardButton.visible → accessed_item is Some("visible") → skipped
    // hardwareKeyboardAvailable → declared property → in scope
    assert!(
        errors.is_empty(),
        "showKeyboardButton.visible = hardwareKeyboardAvailable should produce no errors, got: {errors:?}"
    );
}

// ─── Checker: onInvalidChanged is an unknown signal handler ──────────────────

/// `onInvalidChanged` has no corresponding signal or property named `invalid`
/// in `Rectangle`, so the checker must report `UnknownSignalHandler`.
#[test]
fn test_on_invalid_changed_is_unknown_signal_handler() {
    let source = r#"
Rectangle {
    onInvalidChanged: {
        console.log("Invalid changed")
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onInvalidChanged")
        ),
        "expected UnknownSignalHandler for onInvalidChanged, got: {errors:?}"
    );
}

/// `onHeightChanged` IS valid for Rectangle (height is a known property).
#[test]
fn test_on_height_changed_is_valid_signal_handler() {
    let source = r#"
Rectangle {
    onHeightChanged: {
        console.log("Height changed")
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onHeightChanged")
        ),
        "onHeightChanged should be valid for Rectangle, got: {errors:?}"
    );
}

/// `onWidthChanged` IS valid for Rectangle.
#[test]
fn test_on_width_changed_is_valid_signal_handler() {
    let source = r#"
Rectangle {
    function onWidthChanged() {
        console.log("Width changed")
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onWidthChanged")
        ),
        "onWidthChanged should be valid for Rectangle, got: {errors:?}"
    );
}

/// `onStatussChanged` (typo: double s) has no corresponding signal → error.
#[test]
fn test_on_status_with_typo_is_unknown_signal_handler() {
    let source = r#"
Rectangle {
    function onStatussChanged() {
        console.log("Status changed")
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onStatussChanged")
        ),
        "expected UnknownSignalHandler for onStatussChanged, got: {errors:?}"
    );
}

// ─── Checker: Sub.qml scenario — expected errors and non-errors ──────────────

// ─── Arrow function parameters ────────────────────────────────────────────────

/// `onSelectPoint: (id) => { ... }` — `id` is an arrow param, not undefined.
#[test]
fn test_arrow_param_in_signal_handler_not_undefined() {
    let source = "
Rectangle {
    signal selectPoint(int id)

    onSelectPoint: (id) => {
        previewStatic.selectViewPoint(id)
    }

    Item {
        id: previewStatic
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "id")
        ),
        "arrow param 'id' should not be undefined, got: {errors:?}"
    );
}

/// `.filter((item) => item.x)` — `item` is an arrow param, not undefined.
#[test]
fn test_arrow_param_in_iterator_not_undefined() {
    let source = "
Item {
    property var expositions

    function filterExpositions(pointId) {
        expositions.filter((item) => item.field_point_id != pointId)
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "item")
        ),
        "arrow param 'item' should not be undefined, got: {errors:?}"
    );
}

/// `save: (left, right) => { ... }` inside a callback object — params not undefined.
#[test]
fn test_arrow_params_in_nested_callback_not_undefined() {
    let source = r#"
Item {
    function openDialog() {
        BaseFunctions.showDialog("qrc:/SomeDialog.qml", this, {
            save: (left, right) => {
                someModel.leftVal = left
            }
        })
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "left" || name == "right")
        ),
        "arrow params 'left', 'right' should not be undefined, got: {errors:?}"
    );
}

/// Multiple arrow params in a realistic showDialog call — none should be undefined.
#[test]
fn test_multiple_arrow_params_in_show_dialog_not_undefined() {
    let source = r#"
Item {
    function openLensCalculator() {
        BaseFunctions.showDialog("qrc:/LensCalculatorDialog.qml", this, {
            save: (trial_lens_left, trial_lens_right, distance_left, distance_right) => {
                someModel.trial_lens = LensCalculator.saveData(trial_lens_left, trial_lens_right)
                otherModel.distance = LensCalculator.saveData(distance_left, distance_right)
            }
        }, {someModel: someModel, otherModel: otherModel})
    }
}
"#;
    let errors = check(source);
    let param_names = ["trial_lens_left", "trial_lens_right", "distance_left", "distance_right"];
    for param in param_names {
        assert!(
            !has_error(
                &errors,
                |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == param)
            ),
            "arrow param '{param}' should not be undefined, got: {errors:?}"
        );
    }
}

/// Verifies the concrete scenario from Sub.qml:
/// - `onInvalidChanged` → Problem (UnknownSignalHandler)
/// - `onWidthChanged`   → No problem
/// - `onHeightChanged`  → No problem (inline handler)
/// - `internalElement` as a child id is in scope inside `onInvalidChanged`
///   (accessed as `internalElement.internalElementSub`) → no UndefinedName
#[test]
fn test_sub_qml_signal_handler_scenario() {
    let source = r#"
Rectangle {
    id: root

    onHeightChanged: {
        console.log("Height changed")
    }

    onInvalidChanged: {
        console.log("Invalid changed")
        internalElement.internalElementSub = true
    }

    function onWidthChanged() {
        console.log("Width changed")
    }

    Item {
        id: internalElement
        property bool internalElementSub: false
    }
}
"#;
    let errors = check(source);

    // onInvalidChanged MUST be flagged
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onInvalidChanged")
        ),
        "expected UnknownSignalHandler for onInvalidChanged, got: {errors:?}"
    );

    // onHeightChanged and onWidthChanged must NOT be flagged
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onHeightChanged")
        ),
        "onHeightChanged should be valid, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onWidthChanged")
        ),
        "onWidthChanged should be valid, got: {errors:?}"
    );

    // internalElement.internalElementSub has accessed_item → no UndefinedName
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "internalElement")
        ),
        "internalElement is a child id and should be in scope, got: {errors:?}"
    );
}

// ─── instanceof: type name after instanceof must not be undefined ─────────────

/// `root.other5 instanceof Item` — `Item` is a type used with `instanceof`,
/// not a variable reference. It must not be flagged as `UndefinedName`.
#[test]
fn test_instanceof_type_not_flagged_as_undefined() {
    let source = r#"
Rectangle {
    id: root
    property var other5: 0

    onHeightChanged: {
        if (!(root.other5 instanceof Item)) {
            console.log("not an item")
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "Item")
        ),
        "type name after instanceof must not be flagged as undefined, got: {errors:?}"
    );
}

/// `property bool visible: root.x instanceof Item` — `Item` must also be
/// ignored when `instanceof` appears in a property value expression.
#[test]
fn test_instanceof_in_property_value_not_flagged() {
    let source = "
Rectangle {
    id: root
    property var x: 0
    property bool flag: root.x instanceof Item
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedPropertyAccess { name, .. } if name == "Item")
        ),
        "instanceof type in property value must not be flagged, got: {errors:?}"
    );
}

// ─── for-loop variable: loop variable must not be undefined ──────────────────

/// `for (const address of rr)` — `address` is the loop variable and must not
/// be flagged as undefined when used inside the loop body.
#[test]
fn test_for_loop_var_not_undefined() {
    let source = "
Item {
    function doLoop() {
        let rr = []
        for (const address of rr) {
            let kk = address
        }
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "address")
        ),
        "for-loop variable 'address' must not be flagged as undefined, got: {errors:?}"
    );
}

// ─── JS object literal key must not be flagged ───────────────────────────────

/// `closed: () => { ... }` inside a showDialog call — `closed` is a JS object
/// property key, not a variable reference, and must not be flagged as undefined.
#[test]
fn test_object_literal_key_not_flagged_as_undefined() {
    let source = r#"
Item {
    function onWidthChanged() {
        BaseFunctions.showDialog("qrc:/Dlg.qml", this, {
            closed: () => {
                console.error("closed")
            }
        }, {})
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "closed")
        ),
        "JS object literal key 'closed' must not be flagged as undefined, got: {errors:?}"
    );
}

// ─── property expression scope check ─────────────────────────────────────────

/// `property bool foo: undeclared.bar` — `undeclared` is not in scope and must
/// be flagged as `UndefinedPropertyAccess`.
#[test]
fn test_undeclared_name_in_property_expression_flagged() {
    let source = "
Rectangle {
    property bool flag: undeclaredObj.someValue && undeclaredObj.otherValue
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedPropertyAccess { name, .. } if name == "undeclaredObj")
        ),
        "undeclared name in property expression should be flagged, got: {errors:?}"
    );
}

/// Declared property used in another property's expression must NOT be flagged.
#[test]
fn test_declared_property_in_expression_not_flagged() {
    let source = "
Rectangle {
    property bool active: false
    property bool flag: active && active
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedPropertyAccess { name, .. } if name == "active")
        ),
        "declared property used in another property expression must not be flagged, got: {errors:?}"
    );
}

// ─── inline handler with () suffix ───────────────────────────────────────────

/// `onEmptySignal(): { … }` — the `()` suffix must be stripped; the handler
/// should be treated as `onEmptySignal`, which is valid when `emptySignal` exists.
#[test]
fn test_inline_handler_with_paren_suffix_valid() {
    let source = r#"
Rectangle {
    signal emptySignal()

    onEmptySignal(): {
        console.log("handled")
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onEmptySignal")
        ),
        "onEmptySignal(): {{}} with () stripped should be valid, got: {errors:?}"
    );
}

// ─── chained member access: only base name scope-checked ─────────────────────

/// `BaseFunctions.FixationControl.DIGITAL_EYE_TRACKING` in a function body —
/// only `BaseFunctions` (base of the chain) should be scope-checked.
/// `DIGITAL_EYE_TRACKING` and `BOTH` must NOT be flagged.
#[test]
fn test_chained_member_access_only_base_checked_in_function() {
    let source = r#"
Item {
    import "qrc:/ts/baseFunctions.mjs" as BaseFunctions
    property var other5: 0

    function doWork() {
        const x = [BaseFunctions.FixationControl.DIGITAL_EYE_TRACKING, BaseFunctions.FixationControl.BOTH].includes(other5.value)
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "DIGITAL_EYE_TRACKING")
        ),
        "DIGITAL_EYE_TRACKING should not be flagged (chain member), got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "BOTH")
        ),
        "BOTH should not be flagged (chain member), got: {errors:?}"
    );
}

/// `BaseFunctions.calculate().INTERNAL_ENUM.INTERNAL_ITEM` in a property expression —
/// only `BaseFunctions` (base of the chain) should be in accessed_properties.
#[test]
fn test_property_expr_call_chain_not_flagged() {
    let source = r#"
Item {
    import "qrc:/ts/baseFunctions.mjs" as BaseFunctions
    property var base_function_var: BaseFunctions.calculate().INTERNAL_ENUM.INTERNAL_ITEM
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedPropertyAccess { name, .. } if name == "INTERNAL_ENUM")
        ),
        "INTERNAL_ENUM (return-value member) must not be flagged, got: {errors:?}"
    );
}

/// `BaseFunctions.getCachedFieldList().map(field => field.name)` — `field` is
/// an arrow function parameter inside map(); it must not be flagged.
#[test]
fn test_arrow_param_inside_map_in_property_expr_not_flagged() {
    let source = r#"
Item {
    import "qrc:/ts/baseFunctions.mjs" as BaseFunctions
    property var fieldNames: BaseFunctions.getCachedFieldList().map(field => field.name)
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedPropertyAccess { name, .. } if name == "field")
        ),
        "arrow param 'field' inside map() must not be flagged, got: {errors:?}"
    );
}

// ─── template literal: plain text tokens must not be flagged ─────────────────

/// Plain text inside `{...}` (without `$`) in a template literal must NOT be
/// flagged as undefined — only `${expr}` interpolations are expressions.
#[test]
fn test_template_literal_plain_text_not_flagged() {
    let source = r#"
Item {
    property var msg: "hello"

    function greet() {
        let who = "world"
        console.log(`Hello ${who} {not_a_var} ready`)
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "not_a_var")
        ),
        "plain text inside {{}} in template literal must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "ready")
        ),
        "plain text 'ready' after template expression must not be flagged, got: {errors:?}"
    );
}

/// `${name}` interpolation inside template literal IS checked for scope.
#[test]
fn test_template_literal_interpolation_is_scope_checked() {
    let source = "
Item {
    function greet() {
        console.log(`value is ${undeclaredVar}`)
    }
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "undeclaredVar")
        ),
        "variable in template ${{}} interpolation must be scope-checked, got: {errors:?}"
    );
}

// ─── catch clause parameter must not be flagged ───────────────────────────────

/// `catch (e) { console.error(e) }` — `e` is a catch parameter, not undefined.
#[test]
fn test_catch_param_not_undefined() {
    let source = r#"
Item {
    function doWork() {
        try {
            console.log("ok")
        } catch (e) {
            console.error("error", e)
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "e")
        ),
        "catch param 'e' must not be flagged as undefined, got: {errors:?}"
    );
}

/// `catch (_)` — underscore catch variable must not be flagged.
#[test]
fn test_catch_underscore_not_undefined() {
    let source = r#"
Item {
    function doWork() {
        try {
            console.log("ok")
        } catch (_) {
            console.error("error")
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "_")
        ),
        "catch param '_' must not be flagged as undefined, got: {errors:?}"
    );
}

// ─── !operator in property value must not cause type mismatch ─────────────────

/// `property bool unlocked: !secureLevel` — `!` converts int to bool,
/// so this must NOT be flagged as a PropertyRefTypeMismatch.
#[test]
fn test_not_operator_bool_conversion_no_type_error() {
    let source = "
Rectangle {
    property int secureLevel
    property bool unlocked: !secureLevel
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyRefTypeMismatch { name, .. } if name == "unlocked")
        ),
        "!secureLevel should not cause PropertyRefTypeMismatch, got: {errors:?}"
    );
}

/// `property bool b: other` (no operator) SHOULD still trigger type mismatch
/// when `other` is declared as `int`.
#[test]
fn test_simple_ref_type_mismatch_still_detected() {
    let source = "
Rectangle {
    property int other: 0
    property bool b: other
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyRefTypeMismatch { name, .. } if name == "b")
        ),
        "simple ref 'other' (int → bool) should still cause PropertyRefTypeMismatch, got: {errors:?}"
    );
}

// ─── Component.onCompleted: attached signal handler ───────────────────────────

/// `Component.onCompleted: { body }` must be parsed and its body
/// scanned for undefined names.
#[test]
fn test_component_on_completed_body_scanned() {
    let source = "
Switch {
    Component.onCompleted: {
        undefinedVar = 5
    }
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "undefinedVar")
        ),
        "undefinedVar inside Component.onCompleted body must be flagged, got: {errors:?}"
    );
}

/// Valid use inside `Component.onCompleted` (accessing a declared property)
/// must NOT be flagged.
#[test]
fn test_component_on_completed_valid_access_not_flagged() {
    let source = "
Switch {
    Component.onCompleted: {
        checked = true
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "checked")
        ),
        "checked (Switch property) inside Component.onCompleted must not be flagged, got: {errors:?}"
    );
}

// ─── Object literal keys not flagged as undefined ─────────────────────────────

/// Keys in an object literal passed to a function `f({key: value})` must not
/// be flagged as undefined names.
#[test]
fn test_inline_object_literal_keys_not_flagged() {
    let source = "
Item {
    property var data: null
    function go() {
        BaseFunctions.switchToHome({currentPatientUuid: data, randomName: data})
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "currentPatientUuid")
        ),
        "object literal key 'currentPatientUuid' must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "randomName")
        ),
        "object literal key 'randomName' must not be flagged, got: {errors:?}"
    );
}

// ─── UnknownType ──────────────────────────────────────────────────────────────

/// A child whose type is not in the Qt DB and not in known_types must be
/// reported as UnknownType when the project context is non-empty.
#[test]
fn test_unknown_type_flagged_when_context_known() {
    use qml_static_analyzer::checker::CheckContext;

    let source = "
Item {
    NonExistentType {
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    // Simulate non-empty known_types (some project types exist)
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeOtherType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownType { type_name } if type_name == "NonExistentType")
        ),
        "NonExistentType must be flagged as UnknownType when known_types is non-empty, got: {errors:?}"
    );
}

// ─── Aliased module imports ────────────────────────────────────────────────────

/// A type whose prefix matches a known import alias (e.g. `Kirigami.Icon` when
/// `import org.kde.kirigami as Kirigami` is present) must NOT be reported as
/// UnknownType — even in a non-empty project context.
#[test]
fn test_aliased_module_type_not_flagged_as_unknown() {
    let source = r#"
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    Kirigami.Icon {
        source: "folder"
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeOtherType".to_string()); // non-empty context
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownType { .. })),
        "Kirigami.Icon (aliased type) must not be reported as UnknownType, got: {errors:?}"
    );
}

/// Multiple different aliases in the same file are all recognized.
#[test]
fn test_multiple_aliases_not_flagged() {
    let source = "
import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import QtQuick.Controls as QQC2

Item {
    Kirigami.FormLayout { }
    PlasmaComponents.ToolButton { }
    QQC2.Label { }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeKnownType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownType { .. })),
        "All aliased types must be accepted without UnknownType errors, got: {errors:?}"
    );
}

/// A type whose prefix is NOT a known alias must still be reported.
#[test]
fn test_non_aliased_dotted_type_still_flagged() {
    let source = "
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    UnknownModule.SomeType { }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeKnownType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownType { type_name } if type_name == "UnknownModule.SomeType")
        ),
        "UnknownModule.SomeType (non-aliased prefix) must still be reported as UnknownType, got: {errors:?}"
    );
}

/// When an aliased type resolves to a known Qt type (e.g. `QQC2.Label` → `Label`),
/// its properties are fully validated — unknown properties must be reported.
#[test]
fn test_aliased_qt_type_properties_validated() {
    let source = r#"
import QtQuick
import QtQuick.Controls as QQC2

Item {
    QQC2.Label {
        text: "hello"
        notARealLabelProp: true
    }
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeKnownType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    // QQC2.Label resolves to Label — valid property must NOT be flagged
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "text")
        ),
        "`text` is a valid Label property and must not be flagged, got: {errors:?}"
    );
    // Invalid property must still be reported
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "notARealLabelProp")
        ),
        "`notARealLabelProp` must be reported as unknown on QQC2.Label, got: {errors:?}"
    );
    // QQC2.Label itself must NOT be reported as UnknownType
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownType { .. })),
        "QQC2.Label must not be reported as UnknownType, got: {errors:?}"
    );
}

// ─── Aliased root element ──────────────────────────────────────────────────────

/// When a file's root element uses an aliased module type (e.g.
/// `PlasmaComponents.ToolButton {` with `import ... as PlasmaComponents`),
/// the base type must be resolved to the Qt type — so its properties are
/// recognised and NOT flagged as unknown.
#[test]
fn test_aliased_root_element_properties_recognised() {
    // ToolButton → Button → AbstractButton → text, checkable, checked, onToggled
    let source = r#"
import QtQuick.Controls as QQC2

QQC2.ToolButton {
    text: "hello"
    checkable: true
    checked: false
    onToggled: console.log("toggled")
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = CheckContext::empty();
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "text")
        ),
        "`text` on aliased root ToolButton must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "checkable")
        ),
        "`checkable` on aliased root ToolButton must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "checked")
        ),
        "`checked` on aliased root ToolButton must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onToggled")
        ),
        "`onToggled` on aliased root ToolButton must not be flagged, got: {errors:?}"
    );
}

/// When a file's root element uses an aliased type whose bare name is NOT in
/// the Qt DB (opaque external type like Kirigami.Page), the properties of
/// that root element must not be reported as UnknownPropertyAssignment.
/// (Opaque base type → qt_props is empty → all assignments are accepted.)
#[test]
fn test_opaque_aliased_root_element_not_flagged() {
    let source = r#"
import org.kde.kirigami as Kirigami

Kirigami.Page {
    title: "My page"
    someKirigamiProp: true
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    // non-empty context
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownPropertyAssignment { .. })),
        "opaque aliased root must not produce UnknownPropertyAssignment, got: {errors:?}"
    );
}

// ─── PropertyTypeMismatch ─────────────────────────────────────────────────────

/// `property int count: "hello"` — string literal assigned to int → PropertyTypeMismatch.
#[test]
fn test_property_type_mismatch_string_to_int() {
    let source = r#"
Rectangle {
    property int count: "hello"
}
"#;
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyTypeMismatch { name, .. } if name == "count")
        ),
        "string literal assigned to int must produce PropertyTypeMismatch, got: {errors:?}"
    );
}

/// `property int count: 42` — correct literal → no PropertyTypeMismatch.
#[test]
fn test_property_type_match_int_no_error() {
    let source = "
Rectangle {
    property int count: 42
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyTypeMismatch { name, .. } if name == "count")
        ),
        "int literal on int property must not produce PropertyTypeMismatch, got: {errors:?}"
    );
}

/// `property bool active: 3.14` — double literal assigned to bool → PropertyTypeMismatch.
#[test]
fn test_property_type_mismatch_double_to_bool() {
    let source = "
Rectangle {
    property bool active: 3.14
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyTypeMismatch { name, .. } if name == "active")
        ),
        "double literal assigned to bool must produce PropertyTypeMismatch, got: {errors:?}"
    );
}

/// `property string rate: 99.9` — double literal assigned to string → PropertyTypeMismatch.
#[test]
fn test_property_type_mismatch_double_to_string() {
    let source = "
Rectangle {
    property string rate: 99.9
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyTypeMismatch { name, .. } if name == "rate")
        ),
        "double literal assigned to string must produce PropertyTypeMismatch, got: {errors:?}"
    );
}

/// `property int code: false` — bool literal assigned to int → PropertyTypeMismatch.
#[test]
fn test_property_type_mismatch_bool_to_int() {
    let source = "
Rectangle {
    property int code: false
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyTypeMismatch { name, .. } if name == "code")
        ),
        "bool literal assigned to int must produce PropertyTypeMismatch, got: {errors:?}"
    );
}

// ─── PropertyRedefinition ─────────────────────────────────────────────────────

/// `property int width: 100` in Rectangle — width already exists in Qt base → PropertyRedefinition.
#[test]
fn test_property_redefinition_of_qt_prop() {
    let source = "
Rectangle {
    property int width: 100
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyRedefinition { name, .. } if name == "width")
        ),
        "redeclaring Qt base property 'width' must produce PropertyRedefinition, got: {errors:?}"
    );
}

/// `property string myCustomProp` — new property name → no PropertyRedefinition.
#[test]
fn test_property_redefinition_new_prop_no_error() {
    let source = r#"
Rectangle {
    property string myCustomProp: "hello"
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::PropertyRedefinition { name, .. } if name == "myCustomProp")
        ),
        "declaring a new property must not produce PropertyRedefinition, got: {errors:?}"
    );
}

// ─── UnknownMemberAccess in function ─────────────────────────────────────────

/// `label.nonExistentProp = "x"` — nonExistentProp is not in Text → UnknownMemberAccess.
#[test]
fn test_unknown_member_access_in_function() {
    let source = r#"
Item {
    Text {
        id: label
        text: "hello"
    }

    function update() {
        label.nonExistentProp = "x"
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownMemberAccess { object, member }
                if object == "label" && member == "nonExistentProp")
        ),
        "assignment to unknown child property must produce UnknownMemberAccess, got: {errors:?}"
    );
}

/// `label.text = "world"` — text IS a property of Text → no UnknownMemberAccess.
#[test]
fn test_known_member_access_no_error() {
    let source = r#"
Item {
    Text {
        id: label
        text: "hello"
    }

    function update() {
        label.text = "world"
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownMemberAccess { object, member }
                if object == "label" && member == "text")
        ),
        "assignment to valid Text.text must not produce UnknownMemberAccess, got: {errors:?}"
    );
}

// ─── MemberAssignmentTypeMismatch ─────────────────────────────────────────────

/// `textField.readOnly = 5` — readOnly is bool, 5 is int → MemberAssignmentTypeMismatch.
#[test]
fn test_member_assignment_type_mismatch_int_to_bool() {
    let source = "
Item {
    TextField {
        id: textField
    }

    function reset() {
        textField.readOnly = 5
    }
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::MemberAssignmentTypeMismatch { object, member, .. }
                if object == "textField" && member == "readOnly")
        ),
        "int assigned to bool member must produce MemberAssignmentTypeMismatch, got: {errors:?}"
    );
}

/// `textField.readOnly = false` — correct bool type → no MemberAssignmentTypeMismatch.
#[test]
fn test_member_assignment_type_match_no_error() {
    let source = "
Item {
    TextField {
        id: textField
    }

    function reset() {
        textField.readOnly = false
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::MemberAssignmentTypeMismatch { object, member, .. }
                if object == "textField" && member == "readOnly")
        ),
        "bool assigned to bool member must not produce MemberAssignmentTypeMismatch, got: {errors:?}"
    );
}

// ─── UnknownPropertyAssignment at root inline level ──────────────────────────

/// `badProp: "hello"` on Rectangle root — unknown property → UnknownPropertyAssignment.
/// Requires non-empty known_types to activate the check.
#[test]
fn test_unknown_inline_assignment_on_root() {
    let source = r#"
Rectangle {
    badProp: "hello"
}
"#;
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "badProp")
        ),
        "unknown inline assignment on root must produce UnknownPropertyAssignment, got: {errors:?}"
    );
}

/// `width: 200` on Rectangle — width IS a valid Qt prop → no UnknownPropertyAssignment.
#[test]
fn test_known_inline_assignment_on_root_no_error() {
    let source = "
Rectangle {
    width: 200
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("SomeType".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownPropertyAssignment { name } if name == "width")
        ),
        "valid Qt property 'width' inline assignment must not be flagged, got: {errors:?}"
    );
}

// ─── UnknownCppMember ─────────────────────────────────────────────────────────

fn make_sensor_ctx() -> CheckContext {
    let mut ctx = CheckContext::empty();
    let mut members = std::collections::HashSet::new();
    members.insert("temperature".to_string());
    members.insert("calibrate".to_string());
    members.insert("sensorCount".to_string());
    ctx.cpp_object_members
        .insert("sensorManager".to_string(), Some(members));
    ctx.cpp_globals.insert("sensorManager".to_string());
    ctx
}

/// `sensorManager.temperature` — temperature IS declared → no UnknownCppMember.
#[test]
fn test_cpp_member_valid_no_error() {
    let source = "
Item {
    function showTemp() {
        console.log(sensorManager.temperature)
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = make_sensor_ctx();
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownCppMember { object, member }
                if object == "sensorManager" && member == "temperature")),
        "valid C++ member access must not produce UnknownCppMember, got: {errors:?}"
    );
}

/// `sensorManager.pressure` — pressure is NOT declared → UnknownCppMember.
#[test]
fn test_cpp_member_unknown_flagged() {
    let source = "
Item {
    function process() {
        let val = sensorManager.pressure
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = make_sensor_ctx();
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(e, ErrorKind::UnknownCppMember { object, member }
                if object == "sensorManager" && member == "pressure")),
        "undeclared C++ member must produce UnknownCppMember, got: {errors:?}"
    );
}

/// Opaque C++ object (None members) — ALL member access allowed, no error.
#[test]
fn test_cpp_opaque_object_allows_all_access() {
    let source = "
Item {
    function doAnything() {
        let x = opaqueService.anyMethod()
    }
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.cpp_object_members.insert("opaqueService".to_string(), None); // opaque
    ctx.cpp_globals.insert("opaqueService".to_string());
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownCppMember { object, .. } if object == "opaqueService")
        ),
        "opaque C++ object must allow all member access, got: {errors:?}"
    );
}

/// `enabled: sensorManager.pressure > 0` (inline assignment, not `property` decl) →
/// UnknownCppMember is checked for inline assignments but NOT for `property T foo: expr`.
#[test]
fn test_cpp_member_unknown_in_inline_assignment() {
    let source = "
Item {
    enabled: sensorManager.pressure > 0
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = make_sensor_ctx();
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(e, ErrorKind::UnknownCppMember { object, member }
                if object == "sensorManager" && member == "pressure")),
        "undeclared C++ member in inline assignment must produce UnknownCppMember, got: {errors:?}"
    );
}

/// `property bool hasPressure: sensorManager.pressure > 0` — C++ member validation does
/// NOT apply to `property T name: expr` declarations (only inline assignments are checked).
///
/// Fixed: the checker now validates `base.member` accesses in `property T name: expr`
/// declarations using the raw expression string stored at parse time.
#[test]
fn test_cpp_member_unknown_in_property_decl_expr() {
    let source = "
Item {
    property bool hasPressure: sensorManager.pressure > 0
}
";
    let file = parse_file("Test", source).expect("parse should succeed");
    let db = load_db();
    let ctx = make_sensor_ctx();
    let errors: Vec<ErrorKind> = qml_static_analyzer::checker::check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(e, ErrorKind::UnknownCppMember { object, member }
                if object == "sensorManager" && member == "pressure")),
        "undeclared C++ member in property decl expr must produce UnknownCppMember, got: {errors:?}"
    );
}

// ─── Declared signal → valid handler ─────────────────────────────────────────

/// `signal mySignal()` declared → `function onMySignal() {}` must be valid.
#[test]
fn test_declared_signal_handler_valid() {
    let source = r#"
Item {
    signal mySignal()

    function onMySignal() {
        console.log("received")
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onMySignal")
        ),
        "handler for declared signal must be valid, got: {errors:?}"
    );
}

/// Handler for a non-declared signal on Item → UnknownSignalHandler.
#[test]
fn test_undeclared_signal_handler_flagged() {
    let source = r#"
Item {
    function onNonExistentSignal() {
        console.log("oops")
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UnknownSignalHandler { handler } if handler == "onNonExistentSignal")
        ),
        "handler with no matching signal must produce UnknownSignalHandler, got: {errors:?}"
    );
}

// ─── Known bugs (ignored): document desired behavior ─────────────────────────

/// `Keys.onPressed: function(event) { event.accepted = true }` — `event` is an inline
/// function parameter and must NOT be flagged as undefined.
///
/// Fixed: the parser now uses `collect_function_keyword_params` to extract parameters
/// from the inline `function(param)` handler syntax, handling the no-space form too.
#[test]
fn test_inline_function_handler_param_not_undefined() {
    let source = "
FocusScope {
    Keys.onPressed: function(event) {
        event.accepted = true
    }
}
";
    let errors = check(source);
    assert!(
        !has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "event")
        ),
        "inline function(param) handler param 'event' must not be flagged, got: {errors:?}"
    );
}

/// `items.map(x => undeclaredThing.doIt(x))` — `undeclaredThing` inside the arrow body
/// is not in scope and SHOULD be flagged as UndefinedName.
///
/// Fixed: `collect_names_from_expression` now recurses into `(args)` in chain calls,
/// so identifiers inside arrow function bodies passed to method calls are scope-checked.
#[test]
fn test_undefined_name_inside_arrow_in_method_call() {
    let source = "
Item {
    property var items: []

    function transform() {
        return items.map(x => undeclaredThing.doIt(x))
    }
}
";
    let errors = check(source);
    assert!(
        has_error(
            &errors,
            |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "undeclaredThing")
        ),
        "undefined name inside arrow in method call must be flagged, got: {errors:?}"
    );
}

// ─── Function parameter: member access must not be validated ─────────────────

/// A function parameter is an untyped JS value. Accessing members on it
/// (even when a child with the same id exists) must NOT produce errors.
/// Regression: `function setClickPointParams(point) { point.backlight = ... }`
/// was incorrectly flagged when a child `id: point` existed.
#[test]
fn test_function_param_member_access_not_flagged() {
    let source = r#"
Rectangle {
    Rectangle { id: point; width: 10 }

    function setClickPointParams(point) {
        point.backlight = true
        point.x = 5
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "point" && member == "backlight"
        )),
        "member access on a function parameter must not be flagged, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownMemberAccess { object, member }
            if object == "point" && member == "backlight"
        )),
        "member assignment on a function parameter must not be flagged, got: {errors:?}"
    );
}

/// Arrow-function parameters in forEach callbacks shadow outer child ids.
/// Accessing members on the arrow-param must NOT produce UnknownQmlMember.
/// Regression: `forEach(element => { element.name = ... })` with `id: element`.
#[test]
fn test_arrow_param_shadows_child_id_member_access_not_flagged() {
    let source = r#"
Item {
    Rectangle { id: element; color: "red" }

    function syncAll(list) {
        list.forEach(element => {
            element.name = "updated"
            element.uuid = "abc"
        })
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "element" && member == "name"
        )),
        "arrow-param member access must not be flagged as UnknownQmlMember, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownMemberAccess { object, member }
            if object == "element" && member == "name"
        )),
        "arrow-param member assignment must not be flagged as UnknownMemberAccess, got: {errors:?}"
    );
}

// ─── Nested function declarations in function body ────────────────────────────

/// A nested `function name() {}` declaration is hoisted to the outer function's
/// local scope. Calling it from the same function must NOT produce UndefinedName.
/// Regression: `function canProceed() { function fieldsValid() {} fieldsValid() }`
#[test]
fn test_nested_function_decl_callable_in_outer_function() {
    let source = r#"
Item {
    property string username: ""
    property bool editing: false

    function canProceed() {
        function fieldsValid() {
            return username !== ""
        }
        function passwordFieldValid() {
            return editing
        }
        return fieldsValid() && passwordFieldValid()
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "fieldsValid"
        )),
        "nested function `fieldsValid` must be callable without UndefinedName, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "passwordFieldValid"
        )),
        "nested function `passwordFieldValid` must be callable without UndefinedName, got: {errors:?}"
    );
}

// ─── Auto-generated <propName>Changed signal is in scope ─────────────────────

/// Every `property T name` declaration implies a `nameChanged` signal.
/// Using `nameChanged()` as a function call to emit it must NOT produce UndefinedName.
/// Regression: `property var pointsModel` → `pointsModelChanged()` was flagged.
#[test]
fn test_auto_generated_changed_signal_not_undefined() {
    let source = r#"
Item {
    property var pointsModel: null
    property string title: ""

    function notifyUpdates() {
        pointsModelChanged()
        titleChanged()
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "pointsModelChanged"
        )),
        "`pointsModelChanged` auto-signal must be in scope, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "titleChanged"
        )),
        "`titleChanged` auto-signal must be in scope, got: {errors:?}"
    );
}

// ─── Sibling functions within a child element block ───────────────────────────

/// Functions declared on the same child element block are mutually callable.
/// Calling a sibling function from another handler must NOT produce UndefinedName.
/// Regression: `function changeItems()` on a child was invisible to sibling handlers.
#[test]
fn test_sibling_function_in_child_element_not_undefined() {
    let source = r#"
Item {
    Rectangle {
        id: panel

        function reloadItems() {
            clearItems()
        }

        function clearItems() {
            console.log("cleared")
        }

        onWidthChanged: {
            reloadItems()
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "clearItems"
        )),
        "sibling function `clearItems` must not be flagged as undefined, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "reloadItems"
        )),
        "sibling function `reloadItems` must not be flagged as undefined in onWidthChanged, got: {errors:?}"
    );
}

// ─── Root-element id: declared signals visible as members ────────────────────

/// When the root element has an id (e.g. `id: dialog`) and declares a signal
/// (`signal exportOnDisk()`), accessing `dialog.exportOnDisk` must NOT produce
/// UnknownQmlMember.
/// Regression: root-id ChildInfo did not include declared signals in function_names.
#[test]
fn test_root_id_signal_accessible_as_member() {
    let source = r#"
Item {
    id: dialog
    signal exportOnDisk()

    Item {
        id: acceptButton

        function accept() {
            dialog.exportOnDisk()
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "dialog" && member == "exportOnDisk"
        )),
        "`dialog.exportOnDisk` (declared signal) must not be flagged, got: {errors:?}"
    );
}

// ─── Qt signals are callable (emittable) from within the element ─────────────

/// Qt signals can be emitted as function calls from within the element's handlers.
/// E.g. `pressAndHold()` inside `onDoubleClicked` on a MouseArea must NOT produce
/// UndefinedName.
/// Regression: qt_signals were not added to child_scope.
#[test]
fn test_qt_signal_emittable_in_handler() {
    let source = r#"
Item {
    MouseArea {
        id: clickArea
        anchors.fill: parent

        onDoubleClicked: {
            pressAndHold()
        }

        onPressAndHold: {
            console.log("held")
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UndefinedName { name, .. } if name == "pressAndHold"
        )),
        "`pressAndHold()` (Qt signal) must be callable without UndefinedName, got: {errors:?}"
    );
}

// ─── Loader own properties accessible via content-proxy id ───────────────────

/// When a Loader has a source, its id maps to the loaded content type in child_id_map.
/// But the Loader's OWN properties (`item`, `status`, `progress`) must still be valid.
/// Regression: `property alias globals: globalsLoader.item` was flagged as UnknownQmlMember.
#[test]
fn test_loader_own_property_item_not_flagged() {
    let source = r#"
Item {
    Loader {
        id: globalsLoader
        source: "qrc:/components/window/Globals.qml"
    }

    property alias globals: globalsLoader.item
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "globalsLoader" && member == "item"
        )),
        "`globalsLoader.item` (Loader property) must not be flagged, got: {errors:?}"
    );
}

// ─── Connections: QML child target with user-declared signals ────────────────

/// When a Connections block targets a QML child (by id) that has declared signals,
/// handlers for those signals must NOT produce UnknownSignalHandler.
/// Regression: only Qt DB signals were checked; user-defined QML signals were ignored.
#[test]
fn test_connections_qml_child_target_user_signal_valid() {
    let source = r#"
Item {
    Item {
        id: navigationPanel
        signal stopPressed()
        signal pausePressed()
    }

    Connections {
        target: navigationPanel
        function onStopPressed() {
            console.log("stopped")
        }
        function onPausePressed() {
            console.log("paused")
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownSignalHandler { handler }
            if handler == "onStopPressed"
        )),
        "`onStopPressed` handler for declared QML signal must be valid, got: {errors:?}"
    );
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownSignalHandler { handler }
            if handler == "onPausePressed"
        )),
        "`onPausePressed` handler for declared QML signal must be valid, got: {errors:?}"
    );
}

/// A handler for a signal that is NOT declared on the target → UnknownSignalHandler.
#[test]
fn test_connections_qml_child_target_undeclared_signal_flagged() {
    let source = r#"
Item {
    Item {
        id: navigationPanel
        signal stopPressed()
    }

    Connections {
        target: navigationPanel
        function onGhostPressed() {
            console.log("ghost")
        }
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownSignalHandler { handler }
            if handler == "onGhostPressed"
        )),
        "`onGhostPressed` has no matching signal on navigationPanel, must be flagged, got: {errors:?}"
    );
}

// ─── Property alias member access ────────────────────────────────────────────

/// `property alias X: Y` — calling `X.member()` where `member` is declared on Y
/// must not be flagged as UnknownQmlMember.
/// Regression: another parsed file may have a root `id: X` of a different type,
/// which was inserted into child_id_map via parent_id_types before the alias was
/// resolved, causing false positives like `Unknown member 'loadPatients' on 'list'`.
#[test]
fn test_property_alias_member_access_resolves_to_aliased_child() {
    let source = r#"
Item {
    property alias list: patientsList

    Item {
        id: patientsList

        function loadPatients() {
            console.log("loading")
        }
    }

    function onTodayModeChanged() {
        list.loadPatients()
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "list" && member == "loadPatients"
        )),
        "`list.loadPatients()` via property alias must not be flagged, got: {errors:?}"
    );
}

/// A property alias for a non-existent member on the aliased target IS still flagged.
#[test]
fn test_property_alias_unknown_member_still_flagged() {
    let source = r#"
Item {
    property alias list: patientsList

    Item {
        id: patientsList

        function loadPatients() {}
    }

    function onSomething() {
        list.ghostMethod()
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(&errors, |e| matches!(e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "list" && member == "ghostMethod"
        )),
        "`list.ghostMethod()` must still be flagged since ghostMethod is not on patientsList, got: {errors:?}"
    );
}

// ─── Fix 1: signal names in parent_id_types ──────────────────────────────────

/// A signal declared on a root element (propagated via parent_id_types) must be
/// callable as a method from any child file that references that element by id.
/// Before the fix `exportDriveChanged()` was missing from function_names and
/// `mainWin.exportDriveChanged()` was incorrectly flagged as UnknownQmlMember.
#[test]
fn test_signal_from_parent_id_types_not_flagged() {
    let source = r#"
Item {
    function emitIt() {
        mainWin.exportDriveChanged()
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    // Simulate mainWin being a root element of another file that declares
    // `signal exportDriveChanged()` — fix adds signals into function_names.
    ctx.parent_id_types.insert(
        "mainWin".to_string(),
        ("ApplicationWindow".to_string(), vec![], vec!["exportDriveChanged".to_string()], false),
    );
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(
            e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "mainWin" && member == "exportDriveChanged"
        )),
        "signal in function_names must not be flagged as UnknownQmlMember, got: {errors:?}"
    );
}

/// An id member that is NOT in function_names must still be flagged.
#[test]
fn test_non_existent_member_on_parent_id_still_flagged() {
    let source = r#"
Item {
    function call() {
        mainWin.ghostSignal()
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    ctx.parent_id_types.insert(
        "mainWin".to_string(),
        ("ApplicationWindow".to_string(), vec![], vec!["exportDriveChanged".to_string()], false),
    );
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(
            e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "mainWin" && member == "ghostSignal"
        )),
        "unknown member must still be flagged, got: {errors:?}"
    );
}

// ─── Fix 2/5: propNameChanged from parent_scopes ─────────────────────────────

/// When a property `worklistData` lives in a parent file and reaches a child
/// via parent_scopes, the auto-generated `worklistDataChanged` signal must also
/// be in scope.  Before the fix it was flagged as UndefinedName.
#[test]
fn test_prop_changed_from_parent_scopes_not_undefined() {
    let source = r#"
Item {
    function notify() {
        worklistDataChanged()
    }
    Connections {
        target: root
        function onWidthChanged() {
            worklistDataChanged()
        }
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    // worklistData comes from a parent file — only the raw name is in parent_scopes;
    // the fix generates the Changed variant automatically.
    let mut parent_scope = std::collections::HashSet::new();
    parent_scope.insert("worklistData".to_string());
    ctx.parent_scopes.insert("Child".to_string(), parent_scope);
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(
            e,
            ErrorKind::UndefinedName { name, .. } if name == "worklistDataChanged"
        )),
        "worklistDataChanged must not be flagged when worklistData is in parent_scopes, got: {errors:?}"
    );
}

/// A completely unknown name must still be flagged even with parent_scopes present.
#[test]
fn test_unknown_name_still_flagged_with_parent_scopes() {
    let source = r#"
Item {
    function broken() {
        totallyMadeUpSignal()
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    let mut parent_scope = std::collections::HashSet::new();
    parent_scope.insert("worklistData".to_string());
    ctx.parent_scopes.insert("Child".to_string(), parent_scope);
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(
            e,
            ErrorKind::UndefinedName { name, .. } if name == "totallyMadeUpSignal"
        )),
        "made-up name must still be flagged, got: {errors:?}"
    );
}

// ─── Fix 3: Qt methods from parent_scopes ────────────────────────────────────

/// Qt methods of a parent file's base type must be accessible in child files
/// through parent_scopes (QML dynamic scoping).  Before the fix `releaseResources()`
/// (a Window Qt method) was flagged as UndefinedName in children of a Window-based
/// file.
#[test]
fn test_qt_method_from_parent_scopes_not_undefined() {
    let source = r#"
Item {
    function cleanUp() {
        releaseResources()
    }
    Connections {
        target: root
        function onWidthChanged() {
            releaseResources()
        }
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    // Simulate the fix: parent_scopes now includes Qt methods from the parent.
    let mut parent_scope = std::collections::HashSet::new();
    parent_scope.insert("releaseResources".to_string());
    ctx.parent_scopes.insert("Child".to_string(), parent_scope);
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(
            e,
            ErrorKind::UndefinedName { name, .. } if name == "releaseResources"
        )),
        "Qt method from parent_scopes must not be flagged, got: {errors:?}"
    );
}

/// A fake Qt method that is NOT in parent_scopes must still be flagged.
#[test]
fn test_fake_qt_method_still_flagged() {
    let source = r#"
Item {
    function broken() {
        absolutelyFakeQtMethod()
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    let mut parent_scope = std::collections::HashSet::new();
    parent_scope.insert("releaseResources".to_string());
    ctx.parent_scopes.insert("Child".to_string(), parent_scope);
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(
            e,
            ErrorKind::UndefinedName { name, .. } if name == "absolutelyFakeQtMethod"
        )),
        "fake method must still be flagged, got: {errors:?}"
    );
}

// ─── Fix 4: property var shadows global id ────────────────────────────────────

/// When a file declares `property var X`, a global id `X` in parent_id_types
/// must NOT be used for member-access validation — the var type means any
/// member access is allowed.  Before the fix `networkDrive.address` was
/// flagged as UnknownQmlMember because a global id `networkDrive` (of type
/// Rectangle) was found in parent_id_types.
#[test]
fn test_property_var_shadows_global_id_no_member_error() {
    let source = r#"
Item {
    property var networkDrive: null

    function configure() {
        networkDrive.address = "192.168.1.1"
        networkDrive.arbitraryField = true
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    use qml_static_analyzer::types::{Property, PropertyType, PropertyValue};
    // Insert a typed global id whose members are known — without the fix this
    // would trigger UnknownQmlMember for `.address` and `.arbitraryField`.
    ctx.parent_id_types.insert(
        "networkDrive".to_string(),
        (
            "Rectangle".to_string(),
            vec![Property {
                name: "name".to_string(),
                prop_type: PropertyType::String,
                value: PropertyValue::Unset,
                line: 0,
                is_simple_ref: false,
                accessed_properties: vec![],
                raw_value_expr: String::new(),
            }],
            vec![],
            false,
        ),
    );
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        !has_error(&errors, |e| matches!(
            e,
            ErrorKind::UnknownQmlMember { object, .. } if object == "networkDrive"
        )),
        "property var must suppress global-id member validation, got: {errors:?}"
    );
}

/// Without a local `property var` declaration the global id is used normally
/// and an unknown member IS flagged.
#[test]
fn test_global_id_member_validation_without_local_property() {
    let source = r#"
Item {
    function call() {
        networkDrive.ghostMember()
    }
}
"#;
    let file = parse_file("Child", source).expect("parse should succeed");
    let db = load_db();
    let mut ctx = CheckContext::empty();
    ctx.known_types.insert("Child".to_string());
    use qml_static_analyzer::types::{Property, PropertyType, PropertyValue};
    ctx.parent_id_types.insert(
        "networkDrive".to_string(),
        (
            "Rectangle".to_string(),
            vec![Property {
                name: "name".to_string(),
                prop_type: PropertyType::String,
                value: PropertyValue::Unset,
                line: 0,
                is_simple_ref: false,
                accessed_properties: vec![],
                raw_value_expr: String::new(),
            }],
            vec![],
            false,
        ),
    );
    let errors: Vec<ErrorKind> = check_file(&file, &db, &ctx)
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        has_error(&errors, |e| matches!(
            e,
            ErrorKind::UnknownQmlMember { object, member }
            if object == "networkDrive" && member == "ghostMember"
        )),
        "without local property var, unknown member must be flagged, got: {errors:?}"
    );
}

// ─── Property JS block name extraction ───────────────────────────────────────

/// An undefined name inside `text: { if (unknownVar) ... }` must be flagged as UndefinedName.
#[test]
fn test_undefined_name_in_property_js_block_flagged() {
    let source = r#"
Item {
    property string text2: ""
    text2: {
        if (undefinedVar2)
            return "yes"
        return "no"
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(&errors, |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "undefinedVar2")),
        "undefined name in property JS block must be flagged, got: {errors:?}"
    );
}

/// A name that IS in scope inside a JS block must not be flagged.
#[test]
fn test_defined_name_in_property_js_block_not_flagged() {
    let source = r#"
Item {
    property bool isActive: false
    property string label: ""
    label: {
        if (isActive)
            return "active"
        return "inactive"
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UndefinedName { name, .. } if name == "isActive")),
        "defined name 'isActive' in property JS block must not be flagged, got: {errors:?}"
    );
}

// ─── parent.member validation ─────────────────────────────────────────────────

/// `parent.left2` — left2 is not an Item property → must produce UnknownQmlMember.
#[test]
fn test_parent_invalid_member_flagged() {
    let source = r#"
Item {
    Rectangle {
        id: inner
        anchors.left: parent.left2
    }
}
"#;
    let errors = check(source);
    assert!(
        has_error(&errors, |e| matches!(e, ErrorKind::UnknownQmlMember { object, member }
            if object == "parent" && member == "left2")),
        "parent.left2 must produce UnknownQmlMember, got: {errors:?}"
    );
}

/// `parent.left` — left IS an Item anchor property → must not produce UnknownQmlMember.
#[test]
fn test_parent_valid_anchor_member_not_flagged() {
    let source = r#"
Item {
    Rectangle {
        id: inner
        anchors.left: parent.left
    }
}
"#;
    let errors = check(source);
    assert!(
        !has_error(&errors, |e| matches!(e, ErrorKind::UnknownQmlMember { object, member }
            if object == "parent" && member == "left")),
        "parent.left must not be flagged as unknown, got: {errors:?}"
    );
}

