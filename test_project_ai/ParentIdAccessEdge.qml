import QtQuick
import QtQuick.Controls

// Edge case: accessing a QML element id that is declared in a PARENT file.
// mainDashboard is the root id of Dashboard.qml (ApplicationWindow).
// Dashboard also declares `property bool processingInBackground`.
// This file tests that member accesses on cross-file parent-scope ids are validated.

BasePanel {
    id: root

    function doWork() {
        // OK: processingInBackground is declared on Dashboard (mainDashboard)
        mainDashboard.processingInBackground = false

        // ERROR: processingInBackground3 does not exist on ApplicationWindow / Dashboard
        mainDashboard.processingInBackground3 = false

        // OK: title is a valid Qt property of ApplicationWindow
        mainDashboard.title = "updated"

        // ERROR: fakeWindowProp does not exist on ApplicationWindow
        mainDashboard.fakeWindowProp = true
    }
}
