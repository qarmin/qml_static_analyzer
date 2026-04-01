import QtQuick
import QtQuick.Controls

ApplicationWindow {
    id: root
    width: 800
    height: 600
    title: "Dashboard"

    ChildPanel {
        id: childPanel
        anchors.fill: parent
    }

    InlineHandlerEdge {
        id: inlineEdge
    }

    StringBraceEdge {
        id: stringEdge
    }

    DataView {
        id: dataView
        width: 200
        height: 200
        anchors.right: parent.right
    }

    InputForm {
        id: inputForm
        anchors.bottom: parent.bottom
        width: parent.width
    }

    ComponentArrayEdge {
        id: compArrayEdge
    }

    TypedParamEdge {
        id: typedParamEdge
    }

    AliasEdge {
        id: aliasEdge
    }

    ConnectionsEdge {
        id: connectionsEdge
    }

    StatesEdge {
        id: statesEdge
    }

    RepeaterEdge {
        id: repeaterEdge
        anchors.left: parent.left
        width: 220
        height: 200
    }

    AttachedEdge {
        id: attachedEdge
    }

    JSEdge {
        id: jsEdge
    }

    NestedScopeEdge {
        id: nestedScopeEdge
    }

    AdvancedForm {
        id: advancedForm
        anchors.top: parent.top
        anchors.right: parent.right
        width: 200
        height: 200
    }

    // ERROR: UnknownType – GhostWidget3000 is not defined anywhere
    GhostWidget3000 {
        id: ghost
    }

    // ERROR: UnknownPropertyAssignment – fakeWindowProperty does not exist on ApplicationWindow
    fakeWindowProperty: true

    // OK: width is a valid property of ApplicationWindow/Window
    minimumWidth: 400

    // ERROR: UndefinedName – unknownRef is not defined anywhere in this scope
    function refresh() {
        unknownRef.reload()         // ERROR: UndefinedName
        dataView.selectedIndex = 0  // OK: selectedIndex is DataView property (custom type → opaque → no error)
        childPanel.itemCount = 5    // OK: opaque custom type access
    }

    // ERROR: UnknownSignalHandler – dataLoaded is a signal from BasePanel/ChildPanel,
    // not from ApplicationWindow, and Dashboard doesn't declare it
    function onDataLoaded() {
        console.log("data loaded from child")
    }

    // OK: onClosing is a valid signal handler for ApplicationWindow
    onClosing: {
        console.log("Window closing")
    }
}
