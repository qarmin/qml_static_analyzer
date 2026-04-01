import QtQuick
import QtQuick.Controls

// Tests for attached properties and handlers (Keys, Component, ListView, etc.)
//
// Expected errors (3):
//   1. UnknownPropertyAssignment – nonExistentAttachedProp is not a property of Item (delegate)
//   2. UndefinedName             – missingService in handleInput()
//   3. UnknownSignalHandler      – FocusScope does not have a `triggered` signal
FocusScope {
    id: root

    property string lastKey: ""
    property bool initialized: false

    // OK: Keys.onPressed is a valid attached handler for any Item/FocusScope
    Keys.onPressed: function(event) {
        lastKey = event.key
        event.accepted = true
    }

    // OK: Keys.onReleased is a valid attached handler
    Keys.onReleased: function(event) {
        console.log("released: " + event.key)
    }

    // OK: Component.onCompleted is valid for any QML element
    Component.onCompleted: {
        initialized = true
        root.forceActiveFocus()
    }

    // OK: Component.onDestruction is valid
    Component.onDestruction: {
        console.log("destroyed")
    }

    ListView {
        id: innerList
        width: 300
        height: 400
        anchors.fill: parent
        model: 10

        delegate: Item {
            id: innerDelegate
            width: ListView.view ? ListView.view.width : 300
            height: 40

            // OK: ListView.isCurrentItem is a valid attached property in a ListView delegate
            property bool isCurrent: ListView.isCurrentItem

            // OK: ListView.view gives access to the enclosing ListView
            property int viewCount: ListView.view ? ListView.view.count : 0

            Text {
                id: itemLabel
                // OK: index is available as a delegate global
                text: "Item " + index
                anchors.centerIn: parent
            }

            // ERROR: UnknownPropertyAssignment – nonExistentAttachedProp is not a property of Item
            nonExistentAttachedProp: "oops"

            MouseArea {
                anchors.fill: parent
                // OK: ListView.currentIndex is a property of the attached ListView object
                onClicked: innerList.currentIndex = index
            }
        }
    }

    // OK: reading a declared property in a function
    function printLastKey() {
        console.log("last key: " + lastKey)
    }

    // ERROR: UndefinedName – missingService is not declared
    function handleInput() {
        missingService.process(lastKey)   // ERROR: UndefinedName
    }

    // ERROR: UnknownSignalHandler – FocusScope does not have a `triggered` signal
    function onTriggered() {
        console.log("triggered")
    }
}
