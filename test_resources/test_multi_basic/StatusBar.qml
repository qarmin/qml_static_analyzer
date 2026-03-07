import QtQuick

Item {
    id: statusBar

    property string message: "Ready"
    property bool busy: false

    function showMessage(msg) {
        message = msg
        busy = false
    }
}

