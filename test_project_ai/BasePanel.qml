import QtQuick
import QtQuick.Controls

// Base component – no errors, used as foundation for ChildPanel
Rectangle {
    id: basePanel

    property color panelColor: "blue"
    property int itemCount: 0
    property string labelText: "Default"
    property bool loading: false

    signal itemSelected(int index)
    signal dataLoaded()
    signal panelClosed()
}
