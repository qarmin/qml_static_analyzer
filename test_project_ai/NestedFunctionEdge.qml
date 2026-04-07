import QtQuick

// Tests for nested function declarations and auto-generated Changed signals.
//
// In JS/QML it is valid to declare a named function inside another function body.
// That name is local to the outer function and must NOT be flagged as undefined
// when called from within that function.
//
// Every `property T name` declaration automatically generates a `nameChanged`
// signal — using `nameChanged()` or `nameChanged` as a reference must be valid.
//
// Expected errors (2):
//   1. UndefinedName – phantomHelper called from outer scope (not declared at file level)
//   2. UndefinedName – missingSignal is not a property or signal on this element

Item {
    id: root

    property var pointsModel: null
    property string activeFilter: ""
    property int retryCount: 0

    // OK: nested named function declaration – `validateInput` and `applyRules`
    // are local to `canProceed` and must be callable without being undefined.
    function canProceed() {
        function validateInput() {
            return activeFilter !== ""
        }
        function applyRules() {
            return retryCount < 3
        }
        return validateInput() && applyRules()
    }

    // OK: `pointsModelChanged` is the auto-generated signal for `property var pointsModel`
    // – emitting it or referencing it must not produce UndefinedName.
    function notifyPointsUpdate() {
        pointsModelChanged()
    }

    // OK: `activeFilterChanged` auto-generated from `property string activeFilter`
    function triggerFilter() {
        activeFilterChanged()
    }

    // OK: deeply nested function declarations
    function outer() {
        function middle() {
            function inner() {
                return 42
            }
            return inner()
        }
        return middle()
    }

    // ERROR: UndefinedName – phantomHelper is not declared anywhere in this file
    function callPhantom() {
        phantomHelper.run()
    }

    // ERROR: UndefinedName – missingSignal is not a real signal on Item
    function checkMissing() {
        missingSignal()
    }
}

