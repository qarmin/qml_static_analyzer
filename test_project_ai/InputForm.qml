import QtQuick
import QtQuick.Controls

Rectangle {
    id: inputForm

    property string inputValue: ""
    property bool isValid: false

    // OK: temperature is a declared property of SensorController
    property double currentTemp: sensorManager.temperature

    // OK: connected is a declared property of SensorController
    property bool sensorConnected: sensorManager.connected

    // ERROR: UnknownCppMember – voltage is not declared in SensorController.h
    // sensorManager.voltage is used in a direct assignment value expression
    enabled: sensorManager.voltage > 0

    // ERROR: UnknownSignalHandler – Rectangle/InputForm doesn't have a `connected` signal
    // (sensorManager.connected is a C++ property, not InputForm's own signal)
    function onConnectedChanged() {
        sensorManager.calibrate()         // OK: calibrate() is Q_INVOKABLE
        let count = sensorManager.sensorCount  // OK: sensorCount is a Q_PROPERTY
    }

    // ERROR: UnknownCppMember – pressure is not declared in SensorController.h
    // ERROR: UndefinedName – undefinedHelper is not in scope
    function processInput() {
        let raw = sensorManager.pressure  // ERROR: UnknownCppMember
        undefinedHelper.process(inputValue)  // ERROR: UndefinedName
        sensorManager.reset()  // OK
    }

    TextField {
        id: textField
        placeholderText: "Enter value"
    }

    Button {
        id: submitBtn
        text: "Submit"
    }

    // ERROR: UnknownMemberAccess – nonExistentField is not a property of TextField
    // ERROR: MemberAssignmentTypeMismatch – readOnly expects bool but got int
    function resetForm() {
        textField.nonExistentField = "x"  // ERROR: UnknownMemberAccess
        textField.readOnly = 5             // ERROR: MemberAssignmentTypeMismatch (readOnly is bool)
        submitBtn.enabled = false          // OK: enabled is bool
        textField.text = ""               // OK: text is a string property
    }
}
