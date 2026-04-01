import QtQuick

// Tests for the Connections element pattern.
//
// Expected errors (3):
//   1. UnknownSignalHandler – onPressureChanged: SensorController has no such signal
//   2. UndefinedName        – undeclaredObj inside onSensorCountChanged handler
//   3. UnknownCppMember     – sensorManager.humidity in readHumidity()
Rectangle {
    id: root

    property bool sensorReady: false
    property double lastTemp: 0.0

    // OK: onTemperatureChanged is a signal declared in SensorController
    Connections {
        target: sensorManager
        function onTemperatureChanged() {
            lastTemp = sensorManager.temperature   // OK: valid Q_PROPERTY
            sensorReady = true
        }
    }

    // OK: onConnectedChanged is a valid signal from SensorController
    Connections {
        target: sensorManager
        function onConnectedChanged() {
            console.log("sensor connected: " + sensorManager.connected)
        }
    }

    // ERROR: UnknownSignalHandler – SensorController does not declare a pressureChanged signal
    Connections {
        target: sensorManager
        function onPressureChanged() {
            console.log("pressure changed")
        }
    }

    // ERROR: UndefinedName – undeclaredObj is not in scope
    Connections {
        target: sensorManager
        function onSensorCountChanged() {
            undeclaredObj.doSomething()   // ERROR: UndefinedName
        }
    }

    // OK: multiple valid calls inside a handler
    Connections {
        target: sensorManager
        function onDeviceFound() {
            sensorManager.calibrate()    // OK: Q_INVOKABLE
            sensorManager.reset()        // OK: Q_INVOKABLE
            let c = sensorManager.sensorCount  // OK: Q_PROPERTY
        }
    }

    // ERROR: UnknownCppMember – humidity is not declared in SensorController.h
    function readHumidity() {
        let h = sensorManager.humidity   // ERROR: UnknownCppMember
        return h
    }
}
