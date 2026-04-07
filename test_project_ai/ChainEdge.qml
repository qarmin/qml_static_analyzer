import QtQuick

// Tests for chained JavaScript operations: .arg(), .filter().map(), nested chains, etc.
//
// Expected errors (4):
//   1. UndefinedName – undefinedArg in qsTr().arg(undefinedArg)
//   2. UndefinedName – ghostFilter passed to items.filter()
//   3. UndefinedName – phantomBase used as chain base
//   4. UndefinedName – missingCallback passed to items.forEach()
Rectangle {
    id: root

    property string label: "test"
    property var items: []
    property int count: 0
    property double ratio: 1.5

    // OK: .arg() with valid in-scope variable
    function showValid() {
        let msg = qsTr("Item count: %1").arg(count)          // OK: count in scope
        let msg2 = qsTr("Ratio: %1 / %2").arg(ratio).arg(count)  // OK: both in scope
        console.log(msg + msg2)
    }

    // ERROR: undefinedArg is not defined anywhere
    function showInvalid() {
        let msg = qsTr("Error: %1").arg(undefinedArg)        // ERROR: UndefinedName
        console.log(msg)
    }

    // OK: valid chained array methods with arrow function params
    function processItems() {
        let filtered = items.filter(x => x > 0).map(x => x * 2)
        let joined = filtered.join(", ")
        console.log("result: " + joined)
    }

    // ERROR: ghostFilter is not in scope
    function processInvalid() {
        let result = items.filter(ghostFilter).map(x => x * 2)  // ERROR: UndefinedName
        return result
    }

    // OK: valid C++ invokable result used in expression
    function checkDevice() {
        let n = deviceManager.scanDevices()          // OK: Q_INVOKABLE
        let name = deviceManager.deviceName          // OK: Q_PROPERTY
        console.log(qsTr("Found %1 devices").arg(n))  // OK: n is in scope
    }

    // ERROR: phantomBase is not defined
    function badChain() {
        let x = phantomBase.getValue().toString()    // ERROR: UndefinedName (phantomBase)
        return x
    }

    // OK: string method chain on a property value
    function formatLabel() {
        let upper = label.toUpperCase()              // OK: label in scope
        return upper.trim()                          // OK: chained string methods
    }

    // OK: C++ property used as .arg() argument after chain
    function buildStatus() {
        let t = sensorManager.temperature
        return qsTr("Temp: %1°C, Devices: %2").arg(t).arg(deviceManager.deviceCount)
    }

    // ERROR: missingCallback is not defined
    function applyCallbacks() {
        items.forEach(missingCallback)               // ERROR: UndefinedName
    }

    // OK: ternary inside .arg() — count and ratio are in scope
    function conditionalArg() {
        let msg = qsTr("Value: %1").arg(count > 0 ? ratio : count)  // OK
        console.log(msg)
    }
}

