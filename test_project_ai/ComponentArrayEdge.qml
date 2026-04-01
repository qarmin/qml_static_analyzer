import QtQuick

// Tests for patterns found in plasma-desktop (ActionMenu.qml / Controller.qml):
//
//   1. `readonly property list<QtObject> __data: [...]` — Component elements
//      separated by `},` must be parsed correctly and their `id:` values made
//      accessible in the enclosing scope.
//
//   2. `function toggle() {}` — empty inline function body.  The parser must
//      NOT consume the element's closing `}` as the function body end.
//
//   3. `readonly property Foo bar: SomeType { ... }` — a property whose value
//      is an inline element.  The element body must be consumed so the outer
//      parser is not confused.
Item {
    id: root

    property bool active: false
    property string status: ""

    signal menuRequested()
    signal dismissed()

    // OK: empty inline function – parser must not absorb the outer `}` as body end
    function toggle() {}

    // OK: typed-parameter inline empty function (QML 6 style)
    function reset(x: var, y: var): void {}

    // ── Component array with `},` separators ──────────────────────────────
    // The three Component ids (menuComp, overlayComp, dialogComp) must be
    // visible to functions declared below this property.
    readonly property list<QtObject> __data: [
        Component {
            id: menuComp
            Item { }
        },

        Component {
            id: overlayComp
            Rectangle {
                color: "transparent"
            }
        },

        Component {
            id: dialogComp
            Item { }
        }
    ]

    // ── Property with inline element value ────────────────────────────────
    // The `Item { … }` body after the `:` must be consumed so the
    // following property / function declarations parse correctly.
    readonly property Item sidePanel: Item {
        visible: false
        width: 200
    }

    property bool sidePanelVisible: sidePanel.visible

    // OK: Component IDs from the __data array are visible here
    function showMenu(): void {
        menuComp.createObject(root)
        overlayComp.createObject(root)
    }

    // OK: toggle() and reset() are callable; they have empty bodies so they
    // do not disturb brace counting.
    onMenuRequested: {
        toggle()
        reset(0, 0)
    }

    onDismissed: {
        status = "dismissed"
    }

    // ERROR: UndefinedName – ghostComp was never declared anywhere
    function showGhost(): void {
        ghostComp.createObject(root)       // ERROR: UndefinedName
    }

    // ERROR: UndefinedName – phantomService is not in scope
    function callPhantom(): void {
        phantomService.invoke(status)      // ERROR: UndefinedName
    }
}
