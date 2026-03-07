import QtQuick
import QtQuick.Controls

Item {
    id: dashboard

    property string currentUser: "guest"
    property bool adminMode: false
    property int totalCards: cardList.cardCount

    signal userChanged(string username)

    function switchUser(username) {
        currentUser = username
        adminMode = false
    }

    function onCardAdded(title) {
        totalCards = cardList.cardCount
    }

    CardList {
        id: cardList
        property string owner: dashboard.currentUser

        Card {
            id: welcomeCard
            property string greeting: "Welcome"
        }

        Card {
            id: infoCard
            property string info: "Info here"
        }
    }
}

