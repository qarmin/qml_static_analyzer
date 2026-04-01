import QtQuick
import QtQuick.Controls

// Tests for ID resolution across nested element scopes.
// All IDs declared anywhere in the component tree should be visible from
// every function and handler within the same file.
//
// Expected errors (3):
//   1. UndefinedName         – phantomBox in badResize()
//   2. UnknownMemberAccess   – panel.notARealProp in badPropAccess()
//   3. UnknownSignalHandler  – Rectangle does not have a `panelReady` signal
Rectangle {
    id: root

    property bool showPanel: true
    property string headerTitle: "Main"

    Rectangle {
        id: panel
        // OK: showPanel is in the enclosing root scope
        visible: showPanel
        color: "white"
        width: 200
        height: 200

        Text {
            id: panelTitle
            text: headerTitle   // OK: headerTitle is in root scope
        }

        Rectangle {
            id: innerBox
            color: "lightgray"
            // OK: panel.width is accessible from a nested child scope
            width: panel.width / 2
            height: 50

            Text {
                id: innerLabel
                // OK: panelTitle is accessible from deep nesting (same file scope)
                text: panelTitle.text + " – inner"
            }
        }
    }

    // OK: panel and its children's IDs are visible from root-level functions
    function collapsePanel() {
        panel.visible = false
        panelTitle.text = "Hidden"   // OK: panelTitle.text is string
    }

    // OK: innerBox is visible from root scope
    function resizeInner() {
        innerBox.width = 100
        innerLabel.text = "resized"
    }

    // OK: reading nested id's property in a binding
    property string innerText: innerLabel.text

    // ERROR: UndefinedName – phantomBox is not declared anywhere in the file
    function badResize() {
        phantomBox.width = 999   // ERROR: UndefinedName
    }

    // ERROR: UnknownMemberAccess – notARealProp is not a property of Rectangle
    function badPropAccess() {
        panel.notARealProp = "x"   // ERROR: UnknownMemberAccess
    }

    // ERROR: UnknownSignalHandler – Rectangle does not have a `panelReady` signal
    function onPanelReady() {
        console.log("panel ready")
    }

    // OK: onWidthChanged is a valid property-change handler for Rectangle (inherits Item)
    onWidthChanged: {
        console.log("root width changed to: " + width)
    }
}
