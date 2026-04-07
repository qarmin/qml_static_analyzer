import QtQuick
import QtQuick.Layouts

// Edge case: value expressions of uppercase-prefixed assignments (like Layout.preferredHeight)
// must be validated, not silently skipped.

Item {
    ColumnLayout {
        // OK: deviceCount is a valid property of deviceManager
        Layout.preferredHeight: deviceManager.deviceCount

        // ERROR: deviceCountTypo does not exist on deviceManager
        Layout.preferredWidth: deviceManager.deviceCountTypo
    }
}
