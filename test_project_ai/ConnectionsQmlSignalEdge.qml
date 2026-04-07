import QtQuick

// Tests for:
//   1. Connections block targeting a QML child that declares a custom signal.
//      The handler name must be derived from that declared signal.
//   2. Qt signals emitted as function calls from within the element's handlers.
//      `pressAndHold()` on a MouseArea inside `onDoubleClicked` must NOT
//      produce UndefinedName.
//
// Expected errors (3):
//   1. UnknownSignalHandler – onGhostSignal has no matching signal on Item (innerItem)
//   2. UndefinedName        – notARealThing used inside onDoubleClicked
//   3. UnknownSignalHandler – onNonExistentCustomSignal is not declared in ChildPanel

Item {
    id: root

    // A plain Item child with a declared custom signal
    Item {
        id: innerItem
        signal customTriggered(string msg)
        signal itemReady()
    }

    // OK: `onCustomTriggered` and `onItemReady` are valid handlers because
    // `innerItem` (an Item child) declares those signals.
    Connections {
        target: innerItem
        function onCustomTriggered(msg) {
            console.log("triggered: " + msg)
        }
        function onItemReady() {
            console.log("ready")
        }
    }

    // ERROR: UnknownSignalHandler – innerItem has no `ghostSignal`
    Connections {
        target: innerItem
        function onGhostSignal() {
            console.log("nope")
        }
    }

    // OK: `pressAndHold()` is a Qt signal on MouseArea – calling it to emit
    // the signal from inside `onDoubleClicked` must NOT produce UndefinedName.
    MouseArea {
        id: clickArea
        anchors.fill: parent

        onDoubleClicked: {
            pressAndHold()              // OK: emit Qt signal as a function call
        }

        onPressAndHold: {
            console.log("held")
        }
    }

    // ERROR: UndefinedName – notARealThing is not in scope
    MouseArea {
        id: badArea
        anchors.fill: parent

        onDoubleClicked: {
            notARealThing()             // ERROR: UndefinedName
        }
    }
}

