import QtQuick
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import QtQuick.Controls as QQC2

// Tests for aliased module imports and QtQuick.Shapes types.
//
// Aliased types whose bare name is in the Qt DB (e.g. QQC2.Label → Label)
// are fully validated — unknown properties on them ARE reported.
//
// Aliased types whose bare name is NOT in the Qt DB (e.g. Kirigami.Icon)
// are treated as opaque external types — no error at all.
//
// Shape / ShapePath (imported directly via QtQuick.Shapes, not aliased) must
// be recognised as valid Qt types.
//
// Expected errors (2):
//   1. UnknownPropertyAssignment – notARealQQC2Prop on QQC2.Label
//   2. UndefinedName             – ghostVar in PlasmaComponents.ToolButton handler
Item {
    id: root

    property string title: "Aliased types test"

    // OK: Kirigami.FormLayout — aliased, not in Qt DB, treated as opaque
    Kirigami.FormLayout {
        anchors.fill: parent
    }

    // OK: Kirigami.Icon — same, opaque
    Kirigami.Icon {
        source: "folder"
        width: 32
        height: 32
    }

    // OK: QQC2.Label → resolves to Label (in Qt DB); `text` is a valid Label property
    QQC2.Label {
        text: title
        // ERROR: UnknownPropertyAssignment — not a real Label property
        notARealQQC2Prop: true
    }

    // OK: PlasmaComponents.ToolButton → aliased, not in Qt DB, opaque
    PlasmaComponents.ToolButton {
        text: "Click me"
        // ERROR: UndefinedName – ghostVar is not declared
        onClicked: ghostVar.doSomething()
    }

    // OK: Shape and ShapePath come from `import QtQuick.Shapes` (no alias)
    Shape {
        anchors.fill: parent

        ShapePath {
            strokeWidth: 2
            strokeColor: "red"
            fillColor: "transparent"
            PathPolyline {
                id: penPath
            }
        }
    }
}
