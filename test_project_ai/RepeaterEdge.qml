import QtQuick
import QtQuick.Controls

// Tests for Repeater delegates and model scope.
//
// Expected errors (3):
//   1. UnknownPropertyAssignment – nonExistentProp is not a property of Rectangle (delegate root)
//   2. UndefinedName             – shadowService in Component.onCompleted
//   3. UndefinedName             – phantomRepeater in resetRepeater()
Rectangle {
    id: root

    property var dataModel: ["alpha", "beta", "gamma"]

    Column {
        id: listColumn
        spacing: 4
        anchors.fill: parent

        Repeater {
            id: itemRepeater
            model: root.dataModel

            delegate: Rectangle {
                id: delegateRect
                width: 200
                height: 40
                // OK: index is a QML delegate global for Repeater
                color: index % 2 === 0 ? "lightblue" : "white"

                // OK: index and modelData are QML delegate globals
                property int myIndex: index
                property string myValue: modelData

                // ERROR: UnknownPropertyAssignment – nonExistentProp is not in Rectangle
                nonExistentProp: "bad"

                Text {
                    id: delegateText
                    // OK: modelData is the implicit string value for string-array model
                    text: modelData
                    anchors.centerIn: parent
                }

                MouseArea {
                    anchors.fill: parent
                    // OK: index and delegateText.text are in scope inside delegate
                    onClicked: console.log("clicked: " + index + " = " + delegateText.text)
                }

                // ERROR: UndefinedName – shadowService is not declared anywhere
                Component.onCompleted: {
                    shadowService.track(index)   // ERROR: UndefinedName
                }
            }
        }
    }

    // OK: accessing the Repeater's count property via its id
    property int totalItems: itemRepeater.count

    // OK: itemAt() is a method of Repeater
    function getFirst() {
        return itemRepeater.itemAt(0)
    }

    // ERROR: UndefinedName – phantomRepeater is not declared
    function resetRepeater() {
        phantomRepeater.model = []   // ERROR: UndefinedName
    }
}
