import QtQuick

// Tests for QML-6 typed function parameters and the `as` type-cast operator.
//
// In QML 6 you can write:
//   function open(x: var, y: var): void { … }
//   function fill(menu: Item, items: var): void { … }
//   property Foo favorites: source.model as FooModel
//
// The `: type` annotations on parameters must not be treated as undefined
// name references, and the return-type annotation (`: void`) must be ignored.
// The `as` keyword in property expressions must not be flagged as undefined.
Item {
    id: root

    property var sourceModel: null
    property var favoritesModel: null
    property string label: ""
    property int maxLen: 80

    signal dataReady()

    // OK: typed params – `x` and `y` are params, `: var` is just annotation
    function open(x: var, y: var): void {
        root.width  = x
        root.height = y
    }

    // OK: two typed params – `text` and `maxLen` must be in scope (not undefined)
    function buildLabel(text: string, limit: var): string {
        return text.substring(0, limit)
    }

    // OK: `as` keyword in property initialiser (QML type cast)
    // `as` must not be flagged as an undefined name.
    property var typedFavorites: sourceModel ? sourceModel as Item : null

    // OK: `as` inside a function expression
    function getTyped(): void {
        let m = sourceModel as Item
        root.favoritesModel = m
    }

    // OK: calling the typed helpers
    onDataReady: {
        label = buildLabel("hello", maxLen)
    }

    // ERROR: UndefinedName – undeclaredProcessor is not in scope at all
    function process(input: var): void {
        undeclaredProcessor.handle(input)   // ERROR: UndefinedName
    }

    // ERROR: UndefinedName – shadowRegistry is not declared
    function register(name: string): void {
        shadowRegistry.add(name)            // ERROR: UndefinedName
    }
}
