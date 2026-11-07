import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    title: Tr.t("한국어")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            Choice {
                Kirigami.FormData.label: Tr.t("Backspace로 지우는 단위:")
                text: Tr.t("자모 단위 (한 → 하 → ㅎ)")
                value: "jamo"
                current: EasimeSettings.backspace
                onPicked: value => EasimeSettings.applyBackspace(value)
            }
            Choice {
                text: Tr.t("글자 단위 (한 → 지움)")
                value: "syllable"
                current: EasimeSettings.backspace
                onPicked: value => EasimeSettings.applyBackspace(value)
            }

            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: Tr.t("글자를 조합하는 중에만 적용됩니다. 이미 입력을 마친 글자는 앱의 Backspace 동작을 따릅니다.")
            }
        }
    }
}
