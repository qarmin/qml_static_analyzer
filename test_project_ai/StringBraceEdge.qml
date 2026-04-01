import QtQuick

// Tests that braces inside string literals don't confuse the parser
Rectangle {
    id: root

    property var data: null
    property string message: ""

    // OK: property with object literal value spanning multiple lines
    property var config: ({
        "mode": "dark",
        "size": 42,
        "enabled": true
    })

    // OK: property with array value
    property var items: [
        "first",
        "second",
        "third"
    ]

    function processJson() {
        // OK: string with braces – parser must not count them
        if (!data || data[0] !== "{") {
            console.log("not json")
            return
        }
        if (data[data.length - 1] !== "}") {
            console.log("unclosed json")
            return
        }
        message = "ok"
    }

    function buildQuery() {
        // OK: template literal with braces in expression
        let q = `SELECT * FROM table WHERE id = ${root.width} AND name = '{'`
        return q
    }

    // ERROR: UndefinedName inside function with string-brace noise
    function parseData() {
        if (data[0] === "{") {
            undefinedParser.parse(data)  // ERROR: UndefinedName
        }
    }
}
