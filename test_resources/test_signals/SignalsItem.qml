import QtQuick

Item {
    id: root

    signal clicked()
    signal valueChanged(int newValue)
    signal textUpdated(string text, bool force)
}

