import QtQuick
import QtQuick.Controls

Rectangle {
    id: root

    property string label: "Click me"
    property bool enabled: true
    property int clickCount: 0

    signal clicked()
    signal longPressed(int duration)

    function reset() {
        clickCount = 0
    }

    function handlePress() {
        clickCount = clickCount + 1
    }
}

