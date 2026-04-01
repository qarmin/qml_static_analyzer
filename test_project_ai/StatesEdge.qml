import QtQuick
import QtQuick.Controls

// Tests for States, Transitions, and PropertyChanges.
//
// Expected errors (3):
//   1. UndefinedName         – undeclaredAnimTarget in jumpTo()
//   2. UnknownSignalHandler  – onStateTransitioned: Rectangle has no such signal
//   3. UndefinedName         – ghostPanel in loadPanel()
Rectangle {
    id: root

    property bool collapsed: false
    property string currentState: ""
    property int panelWidth: 400

    states: [
        State {
            name: "expanded"
            PropertyChanges {
                target: root
                // OK: width is a valid property of Rectangle/Item
                width: panelWidth
                // OK: collapsed is declared above
                collapsed: false
            }
        },
        State {
            name: "collapsed"
            PropertyChanges {
                target: root
                width: 100
                collapsed: true
            }
        }
    ]

    transitions: [
        Transition {
            from: "*"
            to: "expanded"
            NumberAnimation {
                properties: "width"
                duration: 300
            }
        },
        Transition {
            from: "*"
            to: "collapsed"
            NumberAnimation {
                properties: "width"
                duration: 200
            }
        }
    ]

    // OK: set a known state name
    function expand() {
        root.state = "expanded"
        currentState = root.state   // OK: state is a built-in property of Item
    }

    // OK: toggle between states
    function toggle() {
        root.state = (root.state === "expanded") ? "collapsed" : "expanded"
    }

    // ERROR: UndefinedName – undeclaredAnimTarget is not defined anywhere
    function jumpTo() {
        undeclaredAnimTarget.start()    // ERROR: UndefinedName
    }

    // ERROR: UndefinedName – ghostPanel is not declared
    function loadPanel() {
        ghostPanel.visible = true       // ERROR: UndefinedName
    }

    Button {
        id: toggleBtn
        text: root.collapsed ? "Expand" : "Collapse"
        // OK: calling a locally declared function
        onClicked: toggle()
    }

    // ERROR: UnknownSignalHandler – Rectangle does not have a `stateTransitioned` signal
    function onStateTransitioned() {
        console.log("state changed to: " + root.state)
    }

    // OK: onStateChanged is a real built-in signal handler for Item.state
    onStateChanged: {
        currentState = root.state
        console.log("new state: " + root.state)
    }
}
