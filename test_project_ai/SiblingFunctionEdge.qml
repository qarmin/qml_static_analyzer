import QtQuick

// Tests for sibling function calls within a child element block.
//
// A function declared directly on a child element (e.g. `GenericButtonGroup`)
// is callable from all other handlers/functions within that same element block.
// It must NOT be flagged as undefined just because it is not at root scope.
//
// Also tests auto-generated Changed signals on child-level properties.
//
// Expected errors (2):
//   1. UndefinedName – ghostAction called from a sibling handler but not declared
//   2. UndefinedName – outsiderFunction is not defined in this child scope

Item {
    id: root

    // OK: `reloadItems` and `clearItems` are sibling functions on the same child –
    // calling one from the other, or from an onXxx handler, must NOT be flagged.
    Rectangle {
        id: panel
        property bool loaded: false
        property string status: ""

        function reloadItems() {
            clearItems()                    // OK: sibling function
            loaded = false
            statusChanged()                 // OK: auto-generated signal for `property string status`
        }

        function clearItems() {
            status = ""
        }

        onWidthChanged: {
            reloadItems()                   // OK: calling sibling function from Qt signal handler
        }
    }

    // OK: multiple sibling functions, each calling another
    Item {
        id: worker

        function step1() {
            step2()                         // OK: sibling
        }
        function step2() {
            step3()                         // OK: sibling
        }
        function step3() {
            console.log("done")
        }

        Component.onCompleted: {
            step1()                         // OK: sibling function in component-completed handler
        }
    }

    // ERROR: UndefinedName – ghostAction is not declared anywhere
    Item {
        id: broken

        function run() {
            ghostAction()                   // ERROR: UndefinedName
        }
    }

    // ERROR: UndefinedName – outsiderFunction is not in scope for this element
    Item {
        id: isolated
        function check() {
            outsiderFunction()              // ERROR: UndefinedName
        }
    }
}

