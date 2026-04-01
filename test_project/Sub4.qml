import QtQuick
import QtQuick.Controls
import "qrc:/commons/ts/baseFunctions.mjs" as BaseFunctions
import "qrc:/components/window"

SwitchWrapper {
    id: root

    property Item valueItem // No problem, Item is a valid type
    property bool calculateValue: root.checked && root.enabled // No problem, checked and enabled are valid properties of Switch

    property var complexSub4Value: {"very": {"nested": {"object": 5}}, "other": [{"Verry": "nested"}, {"object": 5}]}
    property var complexAgain: complexSub4Value?.address || complexSub4Value?.address // No problem, complexSub4Value is defined and we don't care about parameters, so we don't check if address is a valid property of complexSub4Value
    visible: (complexSub4Value?.address ?? "").length > 0 // No problem, complexSub4Value is defined and we don't care about parameters
    property var susValue: susItem.visible; // No problem, susItem is defined as id of Item, and visible is a valid property of Item

    non_exixiiisiissiissis: 5 // qml-ignore - No problem, disabled by qml ignore comment

    Component.onCompleted: {
        nonnnnn_existend_property = 5; // Problem, nonnnnn_existend_property is not defined anywhere, so it should print error

        console.log(complexSub4Value.other[0].Verry); // No problem, var complexSub4Value is defined, nothing more interest us
        console.log(complexSub4Value.function().Verry2); // No problem, var complexSub4Value is defined, nothing more interest us

        let rr = valueItem; // No problem, valueItem is defined as a property of root, and Item is a valid type
    }
    Component.onCompleted: {
        globalValueAccessible = this // No problem, globalValueAccessible is a property of parent of this component
        complexAgain = qsTrId("PRINTING_JOB_STATE_" + nnnonono_existent.state) // Problem, nnnonono_existent is not defined anywhere, so it should print error

        BaseFunctions.showDialog({
            backupFailed(nonexistent_var) { // No problem, this is passed a function handler, that takes 1 argument, so we don't check if  nonexistent_var exists
                //
            },
        });

        // No problem - points/element are arguments of the functions, so we don't need to check if they exist
        BaseFunctions.getManualKineticPoints()
            .then(function (points) {
            points.forEach(function (element) {
                element.name = element.name2
            })
            non_existenddddddddddddddddddddddddddd = 12 // Problem, non_existenddddddddddddddddddddddddddd is not defined anywhere, so it should print error
            non_existenddddddd2 = 12 // qml-ignore - No problem, disabled by qml ignore comment
        })
    }
    Item {
        Item {
            Text {
                Component.onCompleted: {
                    globalValueAccessible = this // No problem, globalValueAccessible is a property of parent(quite deep) of this component
                }
            }
        }
    }

    ListView {
        onContentYChanged: { // No problem, onContentYChanged is a valid signal handler for contentY property of ListView(which have as base Flickable)
            console.error("Content Y changed: " + contentY); // No problem, contentY is a valid property of ListView
        }
        visible: (complexSub4Value?.address ?? "").length > 0 // No problem: `length` is a member of the string result, not a standalone name
    }

    Behavior on height { // No problem, needs to be only validated that height is a valid property of Switch
        SmoothedAnimation {
            duration: animationTime // Problem animationTime is not defined anywhere, so it should print error
        }
    }

    Loader {
        Item {
            TabBar {
                visible: !susItem.visible // No problem, inside file, accessing any valid id is allowed, so we don't need to follow parent chain visibility here
                enabled: susItem.non_exxxsi
                Component.onCompleted: {
                    currentIndex = 1; // No problem, currentIndex is a valid property of TabBar
                    currentIndex222 = 1; // Problem is that currentIndex222 is not a valid property of TabBar, so it should print error2
                    susItem.visible = true; // No problem, inside file, accessing any valid id is allowed, so we don't need to follow parent chain visibility here
                    susItem.non_eexxxiss = false // Problem, non_eexxxiss is not valid susItem property, so it should print error
                }
            }
        }
    }

    Item {
        Item {
            id: susItem
        }
    }

    Sub5 {

    }
}