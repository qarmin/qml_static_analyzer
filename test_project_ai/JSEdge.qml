import QtQuick

// Tests for modern JavaScript patterns used inside QML functions.
// The parser must handle arrow functions, destructuring, for-of, try/catch,
// template literals and spread without incorrectly flagging their syntax as
// undefined names.
//
// Expected errors (2):
//   1. UndefinedName – undeclaredUtil in transform()
//   2. UndefinedName – ghostRegistry in registerAll()
Item {
    id: root

    property var items: []
    property string summary: ""

    // OK: arrow function stored in a var property
    property var mapper: (x) => x * 2

    // OK: function using destructuring assignment and for-of loop
    function summarize() {
        let total = 0
        for (const item of items) {
            const { value, label } = item
            total += value
            console.log(label)
        }
        summary = "Total: " + total
    }

    // OK: try / catch / finally – all three keywords must stay out of scope checks
    function safeParse(raw) {
        try {
            let parsed = JSON.parse(raw)
            return parsed
        } catch (e) {
            console.error("parse error: " + e.message)
            return null
        } finally {
            console.log("done parsing")
        }
    }

    // OK: template literal with embedded expressions
    function buildMessage(name, count) {
        return `Hello ${name}, you have ${count} messages`
    }

    // OK: spread operator in array literal
    function mergeArrays(a, b) {
        return [...a, ...b]
    }

    // OK: standard JS globals (Math, Array, Object, JSON) must not be flagged
    function clamp(val, min, max) {
        return Math.min(Math.max(val, min), max)
    }

    function getKeys(obj) {
        return Object.keys(obj)
    }

    function serialize(obj) {
        return JSON.stringify(obj)
    }

    // OK: optional chaining and nullish coalescing (modern JS)
    function safeName(obj) {
        return obj?.name ?? "unknown"
    }

    // OK: async/await syntax — present in some QML runtimes; parser must not misread
    async function fetchData(url) {
        let result = await Promise.resolve(42)
        return result
    }

    // ERROR: UndefinedName – undeclaredUtil is not in scope
    function transform() {
        return items.map(x => undeclaredUtil.process(x))   // ERROR: UndefinedName
    }

    // ERROR: UndefinedName – ghostRegistry is not declared anywhere
    function registerAll() {
        items.forEach(item => ghostRegistry.add(item))     // ERROR: UndefinedName
    }
}
