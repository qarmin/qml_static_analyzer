import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/components/window"

Rectangle {
    id: input
    property bool valid: mandatory ? (input.acceptableInput && input.length > 0) : (input.acceptableInput || input.length == 0) // No problem, acceptableInput and length are valid properties of TextField, but we don't care about the type of input, so we don't check if it's a TextField or not
    property bool mandatory: false
    property var pointsCollected: [{"stimulus_size": 5}, {"stimulus_size": 10}] // No problem, var is a valid type for property, and nothing more interest us about this property
    Item {
        Item {
            property real ppi: Math.PI // No problem, Math.PI comes from JavaScript standard library, so anything on Math is valid
            Text {
                Component.onCompleted: {
                    globalValueAccessible = this // No problem, globalValueAccessible is a property of parent(quite deep) of this component
                }
            }
        }
    }

    function randddom() {
        const stimulusSize = pointsCollected[0].stimulus_size // No problem, we only check if pointsCollected is defined - we don't care about its content
        const something = internalElement; // Problem - internalElement is child of Global component, but not parent of this component, so it should print error
    }

    RoundButton {
        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Enter || event.key === Qt.Key_Return) // No problem - everything starting with Qt is treated as valid
                return;
        }
    }

    Rectangle {
        id: input
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            delegate: Rectangle {
                border.width: 10
                radius: 20
                color: "transparent"
                non_existtttttttend: 2 // Problem - non_existtttttttend is not a valid property of Rectangle, so it should print error
            }
            // No problem, ListView is a valid type, and everything inside it is valid as well, we don't care about the type of model or the properties of the delegate
            onContentYChanged: {
                Qt.inputMethod.hide()
                if (contentHeight > height && (height + contentY) > contentHeight + 20) {
                    BaseFunctions.updateListModel(patientsModel, (selectedPatientModel ? selectedPatientModel.uuid : null), search.filter)
                        .then((patientsAvailable) => {
                            patientsAvailable.something(); // No problem, patientsAvailable is defined as a parameter of the function, and we don't care about its type or content
                    })

                    BaseFunctions.updateListModel().then(function (patientsAvailable) {
                        patientsAvailable.something(); // No problem, patientsAvailable is defined as a parameter of the function, and we don't care about its type or content
                    })
                }
            }

            ListModel {
                id: patientsModel
            }
        }
    }

    // No problem - states are valid component and should not run any error
    states: [
        State {
            name: "disable"
            when: !input.enabled
            PropertyChanges {
                target: input
                height: 100 // No problem, height is a valid property of Rectangle, which is the type of input
            }
        },
        State {
            name: "enabled"
            when: input.enabled
            PropertyChanges {
                target: input
                non_eessxistent_property: 2 // Problem - non_eessxistent_property is not a valid property of Rectangle, so it should print error
            }
        },
    ]

    Row {
        Text {

        }
        TextWrapper {
            text: "Random" // No problem, text is a valid property of TextWrapper
            onTextChanged: { // No problem, onTextChanged is a valid signal handler for textChanged signal of TextField - base type of TextWrapper
                console.log("Text changed to: " + text); // No problem, text is a valid property of TextWrapper
            }

            background: Rectangle {
                RoundButton {
                    onClicked: {
                        texttttBusy = false; // No problem, textBusy is a valid property of TextWrapper, and footer is a valid property of TextWrapper, so we can access textBusy from here
                        texttttBusy2 = true; // Problem, textBusy2 is not defined anywhere, so it should print error
                        textColor = "red"; // No problem, textColor is a valid property of TextWrapper, so we can access it from here
                        windowBusy = true // No problem, windowBusy is a valid property of Global which have base WindowBase with such property
                    }
                }
            }
        }
    }

    Button {
        id: button
        icon.source: nonexxxxxssrrr // Problem - nonexxxxxssrrr is not defined anywhere, so it should print error
    }
}