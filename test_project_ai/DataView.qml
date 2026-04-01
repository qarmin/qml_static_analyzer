import QtQuick
import QtQuick.Controls

ListView {
    id: dataView

    property int selectedIndex: -1
    property bool showHeader: true

    // ERROR: UnknownSignalHandler – typo: countTChanged vs countChanged
    // count is a property of Flickable (ListView base), so onCountChanged is valid but not onCountTChanged
    function onCountTChanged() {
        console.log("count changed")
    }

    // OK: count is a valid property of ListView/Flickable
    function onCountChanged() {
        console.log("New count: " + count)
    }

    // ERROR: UndefinedPropertyAccess – undefinedModelCount not in scope
    property bool isEmpty: undefinedModelCount > 0

    // OK: selectedIndex is declared above
    property bool hasSelection: selectedIndex >= 0

    delegate: Item {
        id: delegateRoot

        // OK: index, model, modelData are QML delegate globals
        property int myIndex: index
        property var rowData: modelData

        Text {
            id: labelItem
            text: model.name  // OK: model is a delegate global
        }

        // ERROR: UnknownPropertyAssignment – nonExistentDelegateProp is not a property of Item
        nonExistentDelegateProp: 42

        Component.onCompleted: {
            // ERROR: UnknownMemberAccess – nonExistentTextProp is not a property of Text
            labelItem.nonExistentTextProp = "bad"

            // OK: text is a valid property of Text
            labelItem.text = "updated"
        }
    }
}
