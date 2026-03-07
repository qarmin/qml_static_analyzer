import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/components/window"

Switch {
    id: root

    property bool anyValue: false

    property var valuuuue: null
    property var calulatedValue: (valuuuue?.proper ?? {}).itemInternal  + " (ePDF)" // No problem, var valuuuue is defined, we don't care about

    Component.onCompleted: {
        checked = true;
    }

    onCheckedChanged: { // No problem, onCheckedChanged is a valid signal handler for checkedChanged signal
        console.log("Checked state changed to: " + checked);
        BaseFunctions.switchToHome({currentPatientUuid: anyValue, randomName: anyValue}); // No problem, currentPatientUuid and randomName dict keys, that don't have to be defined anywhere
    }

    onCheckedChanged2: { // Problem is that onCheckedChanged2 is not a valid signal handler for checkedChanged signal, so it should print error
        console.log("Checked state changed to: " + checked);
    }
}