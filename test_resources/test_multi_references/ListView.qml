import QtQuick

Item {
    id: listView

    property var model: dataSource.count
    property string selectedItem: dataSource.currentItem
    property bool isLoading: dataSource.loading

    signal selectionChanged(string item)

    function refresh() {
        dataSource.load()
    }

    function clearAll() {
        dataSource.clear()
        selectedItem = "none"
    }
}

