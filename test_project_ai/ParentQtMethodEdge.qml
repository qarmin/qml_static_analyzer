import QtQuick

// Tests that Qt methods of a parent file's base type are propagated into child
// components through parent_scopes, so they can be called without a qualifier.
//
// Dashboard.qml extends ApplicationWindow → Window.
// Window has Qt method `releaseResources()` which is NOT on Item.
// A child component (Item-based) should be able to call `releaseResources()`
// because it reaches the child's scope via parent_scopes from Dashboard.
//
// For reference: the same mechanism lets components inside a Dialog call
// `close()` (Popup.close), `open()` etc. without qualifying the object.
//
// Expected errors (1):
//   1. UndefinedName – absolutelyFakeQtMethod is not a Qt method on Window

Item {
    id: root

    // OK: releaseResources() is a Qt method on Window (Dashboard's base type).
    // It must NOT be flagged as undefined for any child of Dashboard.
    function cleanUp() {
        releaseResources()
    }

    // OK: calling it from inside a Connections handler scope
    Connections {
        target: root
        function onWidthChanged() {
            releaseResources()
        }
    }

    // ERROR: absolutelyFakeQtMethod does not exist on Window/ApplicationWindow
    function broken() {
        absolutelyFakeQtMethod()
    }
}
