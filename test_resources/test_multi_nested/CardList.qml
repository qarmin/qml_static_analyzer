import QtQuick

Item {
    id: cardList

    property int cardCount: 0
    property string listTitle: "Cards"
    property bool empty: cardCount === 0

    signal cardAdded(string title)
    signal cardRemoved(int index)

    function addCard(title, body) {
        cardCount = cardCount + 1
    }

    function removeCard(index) {
        cardCount = cardCount - 1
    }

    Card {
        id: placeholderCard
        property bool placeholder: true
    }
}

