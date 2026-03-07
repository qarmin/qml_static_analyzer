import QtQuick

ExtendedModel {
    id: specialized

    property string tag: "special"
    property int priority: 0
    property bool locked: false

    signal locked(bool state)

    function lock() {
        locked = true
        priority = priority + 1
    }

    function unlock() {
        locked = false
    }

    function applyTag(t) {
        tag = t
    }
}

