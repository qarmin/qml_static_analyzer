import QtQuick

Item {
    id: mainView

    property string pageTitle: "Main"
    property int itemCount: dataSource.count
    property bool ready: !dataSource.loading

    signal pageChanged(string title)

    function initialize() {
        dataSource.load()
    }

    function onDataReady() {
        pageTitle = "Loaded: " + dataSource.currentItem
    }

    DataSource {
        id: dataSource
    }

    ListView {
        id: listView
    }
}

