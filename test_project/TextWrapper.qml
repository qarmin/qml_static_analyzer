import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/commons/ts/enums.mjs" as Enums
import "qrc:/components/window"
import Role 1.0

Text {
    id: input
    property bool texttttBusy: false // No problem, textBusy is a valid property of TextWrapper
    property color textColor
    property var rrr: Role.roman // No problem, Role comes from C++, so we allow all properties of it

    required property int reportIndex

    property var popupPolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside // No problem, Popup.CloseOnEscape and Popup.CloseOnPressOutside are valid properties of Popup

    width: nonnnnnnn_esssistent.getAllMediaSupportedTypes().map(i => ({name: i[0], data: i[1]})) // Problem - nonnnnnnn_esssistent is not defined anywhere
    height: BaseFunctions.checkNetworkDriveStatus(non_exissssssssstent_item) // Problem - non_exissssssssstent_item is not defined anywhere

    signal blurContent(bool blured)

    onBlurContent: function (blured) { // No problem, a little strange way to define a signal handler, but it works and is not a problem
        blur.visible = blured;
        rrr = Popup.CloseOnEscape | Popup.CloseOnPressOutside // No problem, Popup.CloseOnEscape and Popup.CloseOnPressOutside are valid properties of Popup, and bitwise OR operator is valid for color type
    }

    // No problem - very strange looking ternary operator, but it is valid and works
    text: text ?
        input.rrr ?
            ([Enums.ImportStatus.Finished, Enums.ImportStatus.FailedToImport].includes(textColor) ?
                qsTr("Exit") :
                qsTrId("CANCEL_BUTTON")) :
            qsTr("Import") :
        qsTrId("NEXT_BUTTON")

    color: (text?.role ?? 0).something // No problem, another strange looking expression, but completely valid with js/qml

    // No problem - titleText and messageText are properties of ConfirmationDialog(dict items) so should not be validated
    onTextChanged: textChanged => BaseFunctions.showDialog("qrc:/commons/advanced/ConfirmationDialog.qml", this, {
        accepted: () => {
            return;
        }
    }, {
        titleText: qsTr("Change Text"),
        messageText: qsTr("Are you sure that you want to change text %1?").arg(textChanged)
    })

    Component.onCompleted: {
        reportIndex = 12; // No problem, reportIndex is defined as required property of TextWrapper
    }

    TextInput {
        validator: RegularExpressionValidator {
            regularExpression: /[0-9A-Za-z\- \_]+/ // No problem, regex literals must not be tokenized as identifiers
        }
    }

    ComboBox {
        // No problem: JS object array literal — `name` and `data` are object keys, not QML property assignments
        model: [{
            name: qsTr("Hourly"),
            data: {
                interval: 1000 * 60 * 60
            }
        }, {
            name: qsTr("Daily"),
            data: {
                interval: 1000 * 60 * 60 * 24
            }
        }]
    }

    ListView {
        id: list
        property var validItem: ListView.Contain // No problem, ListView.Contain is a valid contant/enum value of ListView
        Component.onCompleted: {
            var validdddd = ListView.Contain // No problem, ListView.Contain is a valid contant/enum value of ListView
        }
    }

    Item {
        transform: Rotation { origin.x: width/2; origin.y: height/2; angle: 45 } // No problem, Rotation is a valid qml type, which has properties origin and angle, and width and height are valid properties of Item
    }
}