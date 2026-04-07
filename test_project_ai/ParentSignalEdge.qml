import QtQuick

// Tests that a signal declared on a parent file's root element (via parent_id_types)
// is recognised as a valid callable member of that element's id.
//
// Dashboard.qml declares `signal settingsRequested()` on its root id `mainDashboard`.
// From any child component, `mainDashboard.settingsRequested()` must be valid.
//
// Expected errors (1):
//   1. UnknownQmlMember – mainDashboard.ghostSignal is not declared

Item {
    id: root

    // OK: settingsRequested is an explicit signal declared on mainDashboard
    function openSettings() {
        mainDashboard.settingsRequested()
    }

    // OK: processingInBackgroundChanged is the auto-generated Changed signal
    // for `property bool processingInBackground` on mainDashboard
    function notifyProcessing() {
        mainDashboard.processingInBackgroundChanged()
    }

    // ERROR: ghostSignal is not declared on mainDashboard
    function badCall() {
        mainDashboard.ghostSignal()
    }
}
