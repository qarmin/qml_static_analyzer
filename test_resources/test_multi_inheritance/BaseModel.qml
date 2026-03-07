import QtQuick

Item {
    id: root

    property int value: 0
    property string name: "base"
    property bool active: false

    signal valueChanged(int newValue)

    function increment() {
        value = value + 1
    }

    function reset() {
        value = 0
        active = false
    }
}

