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

            Choice {
                Kirigami.FormData.label: Tr.t("언어 전환 방식:")
                text: Tr.t("영어와 마지막 언어 사이를 오가기 (기본값)")
                value: "toggle_last"
                current: EasimeSettings.switchMode
                onPicked: value => EasimeSettings.applySwitchMode(value)
            }
            Choice {
                text: Tr.t("순서대로 순환")
                value: "cycle"
                current: EasimeSettings.switchMode
                onPicked: value => EasimeSettings.applySwitchMode(value)
            }

            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: Tr.t("전환 키를 한 번 누르면 영어와 마지막으로 쓴 언어 사이를 오갑니다. 한국어와 영어를 섞어 쓸 때 늘 한 번이면 됩니다. Shift를 누른 채 전환 키를 누르면 다음 언어로 갑니다. 순환을 고르면 전환 키가 영어, 한국어, 일본어 순서로 한 바퀴씩 돕니다. 켠 언어가 영어와 하나뿐이면 두 방식이 같습니다.")
            }
        }
    }
}
