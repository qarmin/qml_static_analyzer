import QtQuick

Item {
    Loader {
        id: contentLoader
        source: "qrc:/SomeContent.qml"
    }

    function test() {
        contentLoader.setSource("qrc:/OtherContent.qml")
    }
}
