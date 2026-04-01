import QtQuick
import QtQuick.Controls

// Tests C++ integration with DeviceManager (second C++ object from config.toml).
//
// Expected errors (5):
//   1. UnknownCppMember – deviceManager.batteryLevel (not in DeviceManager.h)
//   2. UnknownCppMember – deviceManager.firmwareVersion (not in DeviceManager.h)
//   3. UnknownCppMember – deviceManager.reboot() (not a method in DeviceManager.h)
//   4. UndefinedName    – peripheralBus in linkBus()
//   5. UnknownSignalHandler – DeviceManager has no `scanCompleted` signal
Rectangle {
    id: root

    property string activeDevice: ""
    property bool scanning: false

    // OK: deviceName is a Q_PROPERTY on DeviceManager
    property string displayName: deviceManager.deviceName

    // OK: deviceCount is a Q_PROPERTY on DeviceManager
    property int count: deviceManager.deviceCount

    // OK: active is a writable Q_PROPERTY on DeviceManager
    property bool isActive: deviceManager.active

    // ERROR: UnknownCppMember – batteryLevel is not declared in DeviceManager.h
    property int battery: deviceManager.batteryLevel

    // ERROR: UnknownCppMember – firmwareVersion is not in DeviceManager.h
    property string firmware: deviceManager.firmwareVersion

    // OK: deviceFound(name) is a signal declared in DeviceManager
    Connections {
        target: deviceManager
        function onDeviceFound(name) {
            activeDevice = name   // OK: activeDevice is declared above
        }
    }

    // OK: onActiveChanged is a valid signal from DeviceManager (activeChanged signal)
    Connections {
        target: deviceManager
        function onActiveChanged() {
            console.log("device active state: " + deviceManager.active)
        }
    }

    // ERROR: UnknownSignalHandler – DeviceManager does not declare a scanCompleted signal
    Connections {
        target: deviceManager
        function onScanCompleted() {
            console.log("scan done")
        }
    }

    // OK: calling Q_INVOKABLE methods
    function startScan() {
        scanning = true
        let n = deviceManager.scanDevices()   // OK: scanDevices() is Q_INVOKABLE, returns int
        deviceManager.connect("00:11:22:33")  // OK: connect() is Q_INVOKABLE
    }

    // OK: disconnect is Q_INVOKABLE
    function stopAll() {
        deviceManager.disconnect()   // OK
        scanning = false
    }

    // ERROR: UnknownCppMember – reboot is not declared in DeviceManager.h
    function rebootDevice() {
        deviceManager.reboot()       // ERROR: UnknownCppMember
    }

    // ERROR: UndefinedName – peripheralBus is not in scope
    function linkBus() {
        peripheralBus.attach(deviceManager)   // ERROR: UndefinedName
    }

    Button {
        id: scanBtn
        text: scanning ? "Scanning..." : "Scan"
        enabled: !scanning
        onClicked: root.startScan()
    }

    Text {
        id: deviceLabel
        text: root.displayName.length > 0 ? root.displayName : "No device"
        anchors.top: scanBtn.bottom
        anchors.topMargin: 8
    }
}
