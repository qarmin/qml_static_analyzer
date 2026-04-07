import QtQuick

// Tests for advanced Connections patterns:
//   - C++ object targets (deviceManager, sensorManager)
//   - QML child id as target
//   - Unknown target → error
//   - property declared inside Connections stays in handler scope
//   - target = null reassignment inside handler (must NOT be a false positive)
//   - inline onXxx: { } style handlers
//   - multiple handlers in one block
//   - invalid signal name on known C++ target
//
// Expected errors (5):
//   1. UnknownConnectionsTarget – unknownThing is not in scope
//   2. UnknownSignalHandler     – onBatteryChanged is not declared in DeviceManager.h
//   3. UndefinedName            – outerUndefined in onDeviceCountChanged handler
//   4. UndefinedName            – ghostVar in onConnectedChanged handler
//   5. UnknownCppMember         – deviceManager.serialNumber not in DeviceManager.h
Rectangle {
    id: root

    property bool deviceActive: false
    property string lastDevice: ""
    property int deviceCount: 0
    property double lastTemp: 0.0
    property bool exportCompleted: false

    // OK: target is a known C++ object; onDeviceNameChanged is a valid signal
    Connections {
        target: deviceManager
        function onDeviceNameChanged() {
            lastDevice = deviceManager.deviceName    // OK: valid Q_PROPERTY
            deviceCount = deviceManager.deviceCount  // OK: valid Q_PROPERTY
        }
    }

    // OK: multiple handlers in one Connections block; both signals are valid
    Connections {
        target: deviceManager
        function onActiveChanged() {
            deviceActive = deviceManager.active      // OK: valid Q_PROPERTY
        }
        function onDeviceCountChanged() {
            outerUndefined.doThing()                 // ERROR: UndefinedName
        }
    }

    // ERROR: UnknownSignalHandler – DeviceManager has no batteryChanged signal
    Connections {
        target: deviceManager
        function onBatteryChanged() {
            console.log("battery changed")           // handler itself is the error
        }
    }

    // ERROR: UnknownConnectionsTarget – unknownThing is not declared anywhere
    Connections {
        target: unknownThing
        function onStateChanged() {
            console.log("state changed")
        }
    }

    // OK: property declared inside Connections is in scope for all handlers
    Connections {
        id: trackerConn
        target: sensorManager
        property var lastValue: 0.0
        property bool hasError: false

        function onTemperatureChanged() {
            lastValue = sensorManager.temperature    // OK: local prop + valid C++ member
            hasError = false                         // OK: local Connections property
            lastTemp = sensorManager.temperature     // OK: parent scope property
        }

        // OK: Component.onDestruction uses local Connections property
        Component.onDestruction: lastValue = 0.0
    }

    // ERROR: ghostVar is not in scope (not local Connections prop, not in parent scope)
    Connections {
        target: sensorManager
        function onConnectedChanged() {
            ghostVar.update()                        // ERROR: UndefinedName
        }
    }

    // OK: inline onXxx: { } style handler; onDeviceFound is a valid signal
    Connections {
        target: deviceManager
        onDeviceFound: {
            let name = deviceManager.deviceName      // OK: valid Q_PROPERTY
            console.log("Found: " + name)
        }
    }

    // OK: sensorManager signals are valid; C++ invokable calls are OK
    Connections {
        target: sensorManager
        function onSensorCountChanged() {
            sensorManager.calibrate()                // OK: Q_INVOKABLE
            let n = sensorManager.sensorCount        // OK: Q_PROPERTY
            console.log("sensors: " + n)
        }
    }

    // OK: target = null inside a handler is valid — `target` is a built-in
    // settable property of every Connections block (was a false positive before)
    Connections {
        id: exportConn
        target: deviceManager
        function onDeviceFound(name) {
            console.log("found: " + name)
            exportCompleted = true
            target = null                            // OK: target is always in scope
        }
    }

    // ERROR: serialNumber is not declared in DeviceManager.h
    function checkSerial() {
        let s = deviceManager.serialNumber           // ERROR: UnknownCppMember
        return s
    }
}

