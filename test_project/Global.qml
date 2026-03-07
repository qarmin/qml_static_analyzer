import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/components/window"

WindowBase {
    id: mainWindow

    signal exportDriveChanged();
    property bool processInBackground: false
    property int intVal
    property var anyVar
    property var globalValueAccessible: false

    property int secureLevel
    property bool unlocked: !secureLevel // No problem, secureLevel is int, but applying ! operator changes it to bool, so unlocked is bool

    signal deleteMe();

    windowBusy: true // No problem, windowBusy is property of WindowBase, and we can set it from this derived type
    property bool windowBusyAdv: windowBusy
    contentItem: Column {
        id: column

        RoundButton {
            anchors.horizontalCenter: parent.horizontalCenter

            visible: !random_non_existent_variable // Problem, random_non_existent_variable is not defined anywhere, so it should print error
            enabled: (courseModel.fixation_control == Enums.FixationControl.BOTH || courseModel.fixation_control == Enums.FixationControl.HEIJL_KRAKAU) // Problem - should print error, because courseModel is not defined anywhere in this file, so it should print error, and also error about Enums which is also not imported/available

            text: qsTr("Remap Blind Spot")

            onClicked: {
                windowBusy = true // No problem, dialogBusy is a valid property of this component, even if we don't care about its type or where it is defined
            }
        }
    }

    function onWidthChanged() {
        let internalVariable = "cos"
        const internalConst = "cos2"

        const val = intVal + anyVar.expression
    }
    function onExportDriveChanged() {

    }

    Sub {
        onEmptySignal(): { // No problem, emptySignal is signal in Sub
            other5 = null; // No problem, other5 is a property in Sub
            return
        }
    }

    Item {
        Component { // No problem, Component is a valid type
            Item {
                Sub4 { // No problem, Sub4 is defined, and needs to be valid, even if is very nested
                    switchWrapperColor: "red" // No problem, switchWrapperColor is a property of Sub4 -> SwitchWrapper
                    non_exxxxxxistend = 5; // Problem, non_exxxxxxisntend is not defined anywhere, so it should print error
                }
            }
        }
    }

    Sub2 {
        onCheckedChanged: { // No problem, onCheckedChanged is a valid signal handler for checkedChanged signal on Switch - base type of Sub2
            console.log("Checked state changed to: " + checked);
        }
    }

    Sub3 { // Problem, Sub3 is not defined anywhere, so it should print error

    }

    onDeleteMe: console.log("deleteMe signal emitted") // No problem, onDeleteMe is a valid signal handler for deleteMe signal

    Item {
        id: internalElement

        function internalFunction() {
            let module = "Global.qml"
            console.info(`${module} ${non_existent} {normal_text} ready in ${BaseFunctions.calculateTimeDifferenceInMs()} ms (system uptime: ${core.uptime()} seconds)`) // Problem, non_existent and core should print error, because not defined

            try {
                console.log("")
            } catch (e) { // No problem e is function parameter
                console.error("Error in internalFunction:", e)
            }

            try {
                console.log("")
            } catch(_) { // No problem, catch without named parameter is valid
                console.error("Error in internalFunction")
            }
        }

        onEmptySignal(): { // Problem, emptySignal is signal in Sub, not this item or root element
            other5 = null; // Problem - other5 is property of Sub, not this item or root element
            return
        }
    }
}