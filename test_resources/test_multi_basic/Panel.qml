import QtQuick

Item {
    id: container

    property int width: 200
    property int height: 100
    property string title: "Panel"
    property var content

    signal titleChanged(string newTitle)

    function setTitle(newTitle) {
        title = newTitle
    }

    Rectangle {
        id: background
        property bool visible: true
    }
}

