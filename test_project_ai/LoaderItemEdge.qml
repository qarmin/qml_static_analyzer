import QtQuick

// Tests for Loader's own properties (`item`, `status`, `source`) accessed via
// the Loader's id when the Loader has a known QML-file content type.
//
// `property alias panel: panelLoader.item` must NOT produce UnknownQmlMember because
// `item` is a proper Qt property of the Loader type, even though the `child_id_map`
// entry for the loader id now points to the loaded content type (ChildPanel).
//
// Expected errors (1):
//   1. UnknownQmlMember – panelLoader.nonExistentLoaderProp is not a Loader property

Item {
    id: root

    Loader {
        id: panelLoader
        source: "qrc:/ChildPanel.qml"
    }

    Loader {
        id: viewLoader
        source: "qrc:/DataView.qml"
    }

    // OK: `item` is a Qt property of Loader
    property alias panel: panelLoader.item

    // OK: reading Loader's own properties
    property bool isLoaded: panelLoader.status === Loader.Ready
    property real loadProgress: panelLoader.progress

    // OK: using Loader signals and properties in a function
    function checkLoader() {
        if (panelLoader.status === Loader.Ready) {
            console.log("panel ready")
        }
        panelLoader.active = false
    }

    // ERROR: UnknownQmlMember – nonExistentLoaderProp is not a property of Loader
    property var broken: panelLoader.nonExistentLoaderProp
}
