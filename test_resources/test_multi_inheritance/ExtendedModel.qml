import QtQuick

BaseModel {
    id: extended

    property string category: "default"
    property double ratio: 1.0
    property var extra

    signal categoryChanged(string newCategory)

    function setCategory(cat) {
        category = cat
    }

    function multiply(factor) {
        value = value * factor
        ratio = ratio * factor
    }
}

