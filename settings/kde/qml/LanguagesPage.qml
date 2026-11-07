import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings

Kirigami.ScrollablePage {
    id: page

    title: qsTr("언어")

    readonly property var enabledCodes: EasimeSettings.languages === ""
        ? []
        : EasimeSettings.languages.split(",")
    readonly property var names: ({ korean: "한국어", japanese: "日本語" })
    readonly property string cycle: {
        var parts = [qsTr("영어")]
        for (var i = 0; i < enabledCodes.length; ++i) {
            parts.push(names[enabledCodes[i]])
        }
        parts.push(qsTr("영어"))
        return parts.join(" → ")
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("쓰는 언어만 켜 두세요. 어느 언어도 꼭 켜야 하는 것은 아닙니다. 전환 키를 누를 때마다 아래 순서대로 바뀌며, 열려 있는 모든 창에 똑같이 적용됩니다.")
        }

        Repeater {
            model: [
                { code: "korean", name: "한국어" },
                { code: "japanese", name: "日本語" }
            ]

            delegate: RowLayout {
                id: row

                required property var modelData
                readonly property int position: page.enabledCodes.indexOf(modelData.code)
                readonly property bool on: position >= 0

                Layout.fillWidth: true

                QQC2.Switch {
                    objectName: "switch-" + row.modelData.code
                    text: row.modelData.name
                    checked: row.on
                    onToggled: {
                        EasimeSettings.setLanguageEnabled(row.modelData.code, checked)
                        checked = Qt.binding(() => row.on)
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                QQC2.ToolButton {
                    icon.name: "go-up"
                    text: qsTr("앞으로")
                    display: QQC2.AbstractButton.IconOnly
                    enabled: row.on && row.position > 0
                    onClicked: EasimeSettings.moveLanguage(row.modelData.code, true)
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: text
                }
                QQC2.ToolButton {
                    icon.name: "go-down"
                    text: qsTr("뒤로")
                    display: QQC2.AbstractButton.IconOnly
                    enabled: row.on && row.position < page.enabledCodes.length - 1
                    onClicked: EasimeSettings.moveLanguage(row.modelData.code, false)
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: text
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            enabled: false
            QQC2.Switch {
                text: "中文"
                checked: false
            }
            QQC2.Label {
                text: qsTr("준비 중")
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.bold: true
            text: page.enabledCodes.length > 0
                ? qsTr("전환 순서: %1").arg(page.cycle)
                : qsTr("켜진 언어가 없어 영어로만 입력합니다.")
        }
    }
}
