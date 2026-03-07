import QtQuick

Item {
    id: root

    property int counter: 0
    property var data: null

    function increment() {
        counter = counter + 1
    }

    function reset() {
        counter = 0
        data = null
    }

    function computeSum(a, b) {
        let result = a + b
        return result
    }

    function onCounterChanged() {
        let msg = counter
        console.log(msg)
    }
}

