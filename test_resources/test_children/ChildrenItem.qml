import QtQuick

Item {
    id: root

    property int value: 10

    Rectangle {
        id: childRect
        property string label: "child"

        function doSomething() {
            let x = label
        }
    }

    Item {
        id: innerItem

        property bool active: false

        Rectangle {
            id: deepRect
            property int depth: 2
        }
    }
}

