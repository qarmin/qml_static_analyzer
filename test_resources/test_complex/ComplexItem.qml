import QtQuick
import QtQuick.Controls

Window {
    id: mainWindow

    signal exportDriveChanged()
    property bool processInBackground: false
    property int intVal
    property var anyVar

    function onWidthChanged() {
        let internalVariable = "cos"
        const internalConst = "cos2"
        const val = intVal + anyVar.expression
    }

    function onExportDriveChanged() {

    }

    Item {
        id: internalElement

        function internalFunction() {
            let x = 10
        }
    }
}

