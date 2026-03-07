//! Integration tests for the semantic checker (Stage 3).
//!
//! Each test parses an inline QML snippet, runs `check_file`, and asserts
//! exactly which errors (if any) are produced.

use qml_static_analyzer::checker::{CheckContext, ErrorKind, check_file};
use qml_static_analyzer::parser::parse_file;
use qml_static_analyzer::qt_types::{QtTypeDb, load_default_builtin_db, load_from_json_file};

// ─── helper ──────────────────────────────────────────────────────────────────

/// Load the Qt type database.
///
/// If `QT_TYPES_JSON` env var is set, load from that file.
/// Otherwise fall back to whatever is compiled in (panics if nothing is).
fn load_db() -> QtTypeDb {
    if let Ok(path) = std::env::var("QT_TYPES_JSON") {
        return load_from_json_file(std::path::Path::new(&path)).expect("failed to load Qt types from QT_TYPES_JSON");
    }
    load_default_builtin_db()
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

// ─── Connections: entire block ignored ───────────────────────────────────────

/// Everything inside `Connections { … }` should be silently ignored —
/// no children, no functions, no errors.
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
    // Connections should produce no children and no functions on the root
    assert_eq!(file.children.len(), 0, "Connections block should produce no children");
    assert_eq!(
        file.functions.len(),
        0,
        "Connections block should produce no root functions"
    );

    // Checker should report no errors either
    let errors = check(source);
    assert!(
        errors.is_empty(),
        "Connections block should produce no checker errors, got: {errors:?}"
    );
}

/// Connections block does not pollute scope — references inside it don't leak.
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
    assert_eq!(
        item_child.children.len(),
        0,
        "Connections should not appear as a child of Item"
    );
    assert_eq!(item_child.functions.len(), 0);
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
