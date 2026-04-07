import QtQuick

// Tests for function parameters and arrow-function parameters.
// A function parameter (or arrow-function parameter) is an untyped JS value —
// members assigned or read on it must NOT be validated against child ids.
//
// Expected errors (2):
//   1. UndefinedName  – ghostHelper is not in scope anywhere
//   2. UnknownQmlMember – realChild.nonExistentProp is not a member of Rectangle

Rectangle {
    id: root

    Rectangle {
        id: point
        width: 10
        height: 10
    }

    Rectangle {
        id: element
        color: "red"
    }

    // OK: `point` is a parameter – assigning any field must NOT produce UnknownQmlMember
    // even though a child with id `point` exists.
    function setClickPointParams(point) {
        point.backlight = true
        point.x = 10
        point.y = 20
        point.rho = 1.0
        point.phi = 0.5
    }

    // OK: arrow-function parameter `element` shadows child id `element` –
    // member access on the parameter must NOT be validated.
    function syncElements(list) {
        list.forEach(element => {
            element.name = "updated"
            element.uuid = "abc"
            element.visible = false
        })
    }

    // OK: `item` parameter in arrow function – not the child id
    function process(items) {
        items.map(item => {
            item.value = 0
        })
    }

    // ERROR: UndefinedName – ghostHelper is not declared
    function callGhost() {
        ghostHelper.doSomething()
    }

    // ERROR: UnknownQmlMember – realChild.nonExistentProp is not a property of Rectangle
    function breakReal() {
        point.nonExistentProp = "x"
    }
}

