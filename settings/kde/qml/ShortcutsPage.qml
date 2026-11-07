import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    title: Tr.t("단축키")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            Choice {
                Kirigami.FormData.label: Tr.t("언어 전환 키:")
                text: Tr.t("한/영 키")
                value: "hangul"
                current: EasimeSettings.toggleKey
                onPicked: value => EasimeSettings.applyToggleKey(value)
            }
            Choice {
                text: Tr.t("오른쪽 Alt")
                value: "right_alt"
                current: EasimeSettings.toggleKey
                onPicked: value => EasimeSettings.applyToggleKey(value)
            }
            Choice {
                text: Tr.t("둘 다 (현재 기본값)")
                value: "both"
                current: EasimeSettings.toggleKey
                onPicked: value => EasimeSettings.applyToggleKey(value)
            }

            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: Tr.t("한/영 키는 누르는 즉시 전환합니다. 오른쪽 Alt는 다른 키를 누르지 않고 눌렀다 뗄 때만 전환하므로, Alt와 다른 키를 함께 누르는 단축키는 그대로 쓸 수 있습니다.")
            }
        }
    }
}
