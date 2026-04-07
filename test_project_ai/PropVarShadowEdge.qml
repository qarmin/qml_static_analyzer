import QtQuick

// Tests that a `property var X` declaration shadows a global id `X` from
// parent_id_types, suppressing member-access validation on that name.
//
// `mainDashboard` is the global id of Dashboard.qml (an ApplicationWindow).
// Normally, `mainDashboard.unknownProp` would be flagged as UnknownQmlMember.
// But when the current file redeclares `property var mainDashboard`, the
// global id is blocked and `var` means any member access is allowed.
//
// Expected errors (1):
//   1. UndefinedName – ghostProvider is not defined anywhere

Item {
    id: root

    // Shadows the global id `mainDashboard` – type is unknown (var).
    property var mainDashboard: null

    // OK: var type → member validation is skipped entirely
    function configureWindow() {
        mainDashboard.address = "localhost"
        mainDashboard.arbitraryField = true
        console.log(mainDashboard.completelyFakeProperty)
    }

    // ERROR: ghostProvider is not defined anywhere
    function broken() {
        ghostProvider.fetch()
    }
}
