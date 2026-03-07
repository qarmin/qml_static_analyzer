import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/components/window"

Rectangle {
    id: root

    property int height: 200 // Problem - height is already defined in Rectangle, so this is a redefinition
    property int other: 20 // No problem, other is a new property
    property int other2: false // Problem - other2 is defined as int, but assigned a bool value
    property int other3: false + 30 // No problem, expression is not in the scope of our checks, so with expressions we assume the type is correct
    property bool other4: other // Problem, other4 is defined as bool, but assigned an int property
    property var other5: other // No problem, var can be assigned any type
    property bool testConnections: core.uptime() && core.iteem && core2.uptime() && core2.iteem // Problem - core and core2 are not declared
    property var base_function_var: BaseFunctions.calculate().INTERNAL_ENUM.INTERNAL_ITEM // No problem, we don't check if calculate exists or not on typescript imports
    property var fieldsNames: BaseFunctions.getCachedFieldList().map(field => field.name) // No problem, field is defined in the scope of the map function, and we don't check if getCachedFieldList exists or not on typescript imports

    width: 400 // No problem, width exists
    Layout.fillHeight: true // No problem, Layout exists
    Layout.fillWidth: 21 // Problem - fillWidth is a bool, not an int
    Layout.notExist: true // Problem - no such property
    invalidProperty: "test" // Problem - no such property

    invalidProperty2: 123 // qml-ignore - No problem, ignored by qml-ignore comment

    signal emptySignal() // No problem, signal definition is valid
    signal testSignal(int value) // No problem, signal definition is valid

    function onTestSignal(value) { // No problem, onTestSignal is a valid signal handler for testSignal
        console.log("Value: " + value); // No problem, console.* is a valid - we don't check if log exists or not
    }

    function random() {
        let x = 10; // No problem, x defined here in function scope - to simplify the tests, we don't check for variable scope - x may be used before this line, but we don't check for that
        const y = 20; // No problem, y defined here in function scope - to simplify the tests, we don't check for variable scope - y may be used before this line, but we don't check for that

        zzzz = x + y; // Problem - zzzz is not defined anywhere in this function or in the component scope or in the global scope
        let pp = y.tw + x.mmm; // No problem - we don't check if properties exist on javascript types
        let ll = other5.something; // No problem - we don't check if properties exist on var types
    }

    function onStatussChanged() { // Problem - onStatusChanged is a signal handler, but there is no such signal in Rectangle
        console.log("Status changed"); // No problem, console.* is a valid - we don't check if log exists or not
        qsTr("Test"); // No problem, qsTr is a valid function - we don't check if it exists or not
        qsTrId("TestId"); // No problem, qsTrId is a valid function - we don't check if it exists or not



        // No problem below - && and several other operators, allows to split expressions into multiple lines
        const enableHeadTracking = BaseFunctions.headTracking &&
            [BaseFunctions.FixationControl.DIGITAL_EYE_TRACKING, BaseFunctions.FixationControl.BOTH].includes(other5.fixation_control);


    }
    function onWidthChanged() { // No problem, onWidthChanged is a signal handler for the width property, which exists
        console.log("Width changed");
        let opvDiskExport = settingsManager.opvDiskExport; // Problem - settingsManager is not defined anywhere in this component or in the global scope

        BaseFunctions.showDialog('qrc:/components/window/LoginErrorDialog.qml', this, {
            closed: () => { // No problem, closed is just a function we pass in an object, we don't check if it's a valid signal or not
                console.error("Dialog closed"); // No problem, console.* is a valid - we don't check if error exists or not
            }
        }, {});
    }

    onHeightChanged: { // No problem, onHeightChanged is a signal handler for the height property, which exists
        console.log("Height changed");
        let rr = [];

        for (const address of rr) {
            let kk = address; // No problem, address is defined in for loop
        }
        for (const [signalName, signalHandler] of rr) { // No error - rr is split into two variables - we don't validate if this have sense
            BaseFunctions.item[signalName].connect(signalHandler) // No problem, we don't check if item or connect exist on BaseFunctions
        }
    }
    onInvalidChanged: { // Problem - onInvalidChanged is a signal handler, but there is no such signal in Rectangle
        console.log("Invalid changed");
        internalElement.internalElementSub = true; // No problem, internalElementSub is a valid property of internalElement
        internalElement.internalElementSub = 0; // Problem, internalElementSub is a bool, but assigned an int value
        internalElement.internalElementSubNonExisting = true; // Problem, internalElementSubNonExisting is not a property of internalElement

        if (!(root.other5 instanceof Item)) { // No problem, instanceof is a valid operator
            console.log("Active focus item is not ShowKeyboardButton");
        }
    }

    property bool visible: root.other5 instanceof Item // No problem, instanceof is valid operator
    onVisibleChanged: if (!visible) visible = true; // No problem, onVisibleChanged is a signal handler for the visible property, which exists

    Item {
        id: internalElement
        property bool internalElementSub: false
    }


    // Entire connections needs to be ignored, except id if exists
    Connections {
        id: czoker
        target: anybody
        function onModulesStateChanged(path) {
            if (czoker.modulesReady) {
                printModuleStartTime("Core modules")
            }
        }
    }

    function czokerFunction() {
        czoker.someProperty = 5; // No problem, we don't check if czoker or someProperty exist
    }
}