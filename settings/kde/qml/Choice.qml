// One radio button of a group where `current` is the saved value. The
// binding is restored after each click, so if saving fails the button goes
// back to what is really saved.
import QtQuick
import QtQuick.Controls as QQC2

QQC2.RadioButton {
    id: control

    property string value
    property string current

    objectName: "choice-" + value

    signal picked(string value)

    checked: control.current === control.value
    onToggled: {
        if (checked) {
            control.picked(control.value)
        }
        control.checked = Qt.binding(() => control.current === control.value)
    }
}
