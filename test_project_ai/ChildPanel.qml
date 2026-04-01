import QtQuick
import QtQuick.Controls

BasePanel {
    id: root

    // ERROR: PropertyRedefinition – height already exists in Item (Qt base)
    property int height: 100

    // ERROR: PropertyTypeMismatch – declared string but assigned int literal
    property string title: 42

    // ERROR: PropertyTypeMismatch – declared bool but assigned double literal
    property bool active: 3.14

    // ERROR: PropertyTypeMismatch – declared int but assigned bool literal
    property int retryCount: false

    // ERROR: PropertyTypeMismatch – declared int but assigned string literal (NEW gap)
    property int errorCode: "FAIL"

    // ERROR: PropertyTypeMismatch – declared string but assigned double literal (NEW gap)
    property string rate: 99.9

    // OK: both int, no mismatch
    property int counter: 10

    // ERROR: PropertyRefTypeMismatch – declared bool but assigned int property `counter`
    property bool isCounterBool: counter

    // OK: loading is bool from BasePanel, same type
    property bool stillLoading: loading

    // ERROR: UnknownPropertyAssignment – unknownProp9876 does not exist on BasePanel/Rectangle
    unknownProp9876: "test"

    // ERROR: UndefinedPropertyAccess – undeclaredNameABC not in scope
    property bool vis: undeclaredNameABC

    // ERROR: UnknownSignalHandler – typo: itemSelecttted vs itemSelected
    function onItemSelecttted() {
        console.log("selected")
    }

    // OK: valid signal handler for itemSelected signal declared in BasePanel
    function onItemSelected(index) {
        console.log("Item selected: " + index)
    }

    // ERROR: UndefinedName – unknownVar3456 not defined anywhere
    function doWork() {
        unknownVar3456 = 5
        let local = counter // OK: counter is in scope
    }

    // OK: valid signal handler for panelClosed signal from BasePanel
    function onPanelClosed() {
        loading = false // OK: loading is a property from BasePanel
    }

    // OK: uses AI_DEBUG_MODE from globals config
    function debugDump() {
        if (AI_DEBUG_MODE) {
            console.log("debug mode active")
        }
    }
}
