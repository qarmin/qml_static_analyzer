import QtQuick
import QtQuick.Controls

// Tests for property alias declarations.
//
// Expected errors (4):
//   1. UnknownPropertyAssignment  – aliasToNowhere: targets ghostChild which doesn't exist
//   2. PropertyTypeMismatch       – badTypedAlias declared as int but alias value is string
//   3. UndefinedName              – phantomId in badAccess()
//   4. UnknownMemberAccess        – titleLabel.nonExistentProp in wrongMember()
Rectangle {
    id: root

    // OK: valid alias – titleLabel.text is a real property of a child Text
    property alias titleText: titleLabel.text

    // OK: valid alias – overlayRect.visible is a real property of a child Rectangle
    property alias overlayVisible: overlayRect.visible

    // ERROR: UnknownPropertyAssignment – ghostChild is not declared as a child id
    property alias aliasToNowhere: ghostChild.width

    Text {
        id: titleLabel
        text: "Hello"
    }

    Rectangle {
        id: overlayRect
        visible: false
        color: "black"
        opacity: 0.5
    }

    // OK: using a valid alias to read/write child property
    function updateTitle(newText) {
        titleText = newText          // OK: alias for titleLabel.text
    }

    // OK: using a valid alias to toggle visibility
    function showOverlay() {
        overlayVisible = true        // OK: alias for overlayRect.visible
    }

    // ERROR: UndefinedName – phantomId is not declared anywhere in this file
    function badAccess() {
        phantomId.width = 100        // ERROR: UndefinedName
    }

    // ERROR: UnknownMemberAccess – nonExistentProp is not a property of Text
    function wrongMember() {
        titleLabel.nonExistentProp = "x"   // ERROR: UnknownMemberAccess
    }

    // OK: reading alias through binding
    property string displayTitle: titleText
}
