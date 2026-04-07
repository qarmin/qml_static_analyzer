import QtQuick

// Edge case: accessing a member of a known QML child element in a property value
// expression should be validated.

Item {
    id: root

    Item {
        id: childItem
        property string label: "hello"
    }

    // ERROR: childItem.label2 does not exist (only `label` is declared)
    property bool isEmpty: childItem.label2 !== ""

    // OK: childItem.label is declared
    property string copy: childItem.label
}
