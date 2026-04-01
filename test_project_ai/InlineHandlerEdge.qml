import QtQuick

// Tests for inline signal handlers with multi-line bodies (no braces)
Rectangle {
    id: root

    property bool ready: false
    property string status: ""

    signal activated()
    signal dataReceived(string payload)

    // OK: single-expression inline handler
    onActivated: console.log("activated")

    // OK: multi-line handler without braces – body on next line
    onWidthChanged:
        if (width > 100)
            console.log("wide: " + width)

    // ERROR: UndefinedName – ghostVar is not in scope in multi-line body
    onHeightChanged:
        if (height > 0)
            ghostVar = height

    // OK: function-form handler with string containing braces – parser must not count them
    // Use function form so `payload` is in scope as a declared parameter
    function onDataReceived(payload) {
        if (!payload || payload[0] !== "{") {
            console.log("not json")
            return
        }
        console.log("received: " + payload)
    }

    // ERROR: UnknownSignalHandler
    function onNonExistentXXXChanged() {
        console.log("bad handler")
    }

    // OK: nested if-else in multi-line handler
    onVisibleChanged:
        if (visible)
            status = "shown"

    // OK: Component.onCompleted with braces containing string with `}`
    Component.onCompleted: {
        let s = "closing brace: }"
        ready = true
    }
}
