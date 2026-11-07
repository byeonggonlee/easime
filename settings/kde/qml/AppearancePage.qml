import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings

Kirigami.ScrollablePage {
    title: qsTr("모양")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.CheckBox {
                id: box

                objectName: "box"

                Kirigami.FormData.label: qsTr("상태 표시:")
                text: qsTr("언어를 바꿀 때 커서 옆에 상자로 알려 주기")
                checked: EasimeSettings.indicatorBox
                onToggled: {
                    EasimeSettings.applyIndicatorBox(checked)
                    checked = Qt.binding(() => EasimeSettings.indicatorBox)
                }
            }

            RowLayout {
                Kirigami.FormData.label: qsTr("표시 시간:")
                enabled: EasimeSettings.indicatorBox

                QQC2.SpinBox {
                    from: 100
                    to: 10000
                    stepSize: 100
                    editable: true
                    value: EasimeSettings.indicatorMs
                    onValueModified: EasimeSettings.applyIndicatorMs(value)
                }
                QQC2.Label {
                    text: qsTr("밀리초 (1000 = 1초)")
                }
            }

            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: qsTr("입력창을 누르거나 창을 옮길 때는 표시하지 않고, 사용자가 언어를 바꿀 때만 잠깐 나타납니다.")
            }
        }
    }
}
