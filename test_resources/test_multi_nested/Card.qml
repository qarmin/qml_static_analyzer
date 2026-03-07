import QtQuick

Rectangle {
    id: card

    property string cardTitle: "Card"
    property string cardBody: ""
    property color backgroundColor: "white"
    property bool elevated: false

    signal cardClicked()

    function setContent(title, body) {
        cardTitle = title
        cardBody = body
    }
}

