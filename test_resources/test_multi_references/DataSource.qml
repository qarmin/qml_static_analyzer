import QtQuick

Item {
    id: dataSource

    property int count: 0
    property string currentItem: "none"
    property bool loading: false

    signal dataReady()
    signal errorOccurred(string message)

    function load() {
        loading = true
    }

    function clear() {
        count = 0
        currentItem = "none"
        loading = false
    }
}

