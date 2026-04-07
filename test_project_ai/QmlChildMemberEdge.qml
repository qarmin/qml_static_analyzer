import QtQuick
import QtQuick.Controls

// Tests for QML child member access validation.
//
// Expected errors (4):
//   1. UnknownQmlMember  – previewItem.nonExistentMethod() – not declared anywhere
//   2. UnknownQmlMember  – previewItem.ghostProperty – not a property of PreviewChild
//   3. UnknownMemberAccess – previewItem.badAssign = 42 (write to unknown member)
//   4. UndefinedName     – phantomChild.doStuff() – phantomChild is not in scope
//
// NOT errors (false-positive checks):
//   - previewItem.update()      – Qt method (Canvas/Item)
//   - previewItem.linesChanged() – auto-signal for property var lines
//   - previewItem.validFunc()   – function declared in the inline child block
//   - previewItem.myProp        – property declared inline
//   - previewItem.myPropChanged – auto-generated signal for myProp
Rectangle {
    id: root

    // Inline child with explicitly declared members
    Item {
        id: previewItem
        property var lines: []
        property int myProp: 0

        function validFunc() {
            console.log("valid")
        }
        function anotherFunc(x) {
            return x * 2
        }
    }

    // OK: Qt method inherited from Item (update is a method on Item)
    function callUpdate() {
        previewItem.update()               // OK: Qt method
    }

    // OK: auto-generated signal for `property var lines`
    function connectLines() {
        previewItem.linesChanged()         // OK: auto-generated signal
    }

    // OK: inline-declared function
    function callValid() {
        previewItem.validFunc()            // OK: declared inline
        previewItem.anotherFunc(5)         // OK: declared inline
    }

    // OK: inline-declared property read and write
    function useMyProp() {
        let v = previewItem.myProp         // OK: declared inline
        previewItem.myPropChanged()        // OK: auto-generated signal
    }

    // ERROR: nonExistentMethod is not declared anywhere
    function callBad() {
        previewItem.nonExistentMethod()    // ERROR: UnknownQmlMember
    }

    // ERROR: ghostProperty is not declared anywhere
    function readGhost() {
        let x = previewItem.ghostProperty  // ERROR: UnknownQmlMember
        console.log(x)
    }

    // ERROR: write to unknown member via assignment
    function writeBad() {
        previewItem.badAssign = 42         // ERROR: UnknownMemberAccess
    }

    // ERROR: phantomChild is not declared in this scope
    function callPhantom() {
        phantomChild.doStuff()             // ERROR: UndefinedName
    }
}

