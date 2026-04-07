import QtQuick

// Tests that an auto-generated `propNameChanged` signal is accessible even when
// the property itself is declared in a parent file (propagated via parent_scopes),
// not in the current file.
//
// Dashboard.qml (the parent) declares `property bool processingInBackground`.
// This means `processingInBackgroundChanged` is also a valid name in the scope
// of any direct or indirect child component of Dashboard.
//
// Expected errors (1):
//   1. UndefinedName – totallyMadeUpSignal is not in scope

Item {
    id: root

    // OK: processingInBackground comes from Dashboard (parent) via parent_scopes.
    // The auto-generated Changed signal must also be in scope.
    function notifyBackgroundWork() {
        processingInBackgroundChanged()
    }

    // OK: calling it inside a Connections handler scope
    Connections {
        target: root
        function onWidthChanged() {
            processingInBackgroundChanged()
        }
    }

    // ERROR: totallyMadeUpSignal is not declared anywhere
    function broken() {
        totallyMadeUpSignal()
    }
}
