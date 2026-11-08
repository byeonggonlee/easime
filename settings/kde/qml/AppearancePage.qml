import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    title: Tr.t("모양")

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

                Kirigami.FormData.label: Tr.t("상태 표시:")
                text: Tr.t("언어를 바꿀 때 커서 옆에 상자로 알려 주기")
                checked: EasimeSettings.indicatorBox
                onToggled: {
                    EasimeSettings.applyIndicatorBox(checked)
                    checked = Qt.binding(() => EasimeSettings.indicatorBox)
                }
            }

            RowLayout {
                Kirigami.FormData.label: Tr.t("표시 시간:")
                enabled: EasimeSettings.indicatorBox

                QQC2.SpinBox {
                    id: duration
                    objectName: "indicator-duration"
                    Accessible.name: Tr.t("표시 시간 (초)")
                    from: 100
                    to: 10000
                    stepSize: 100
                    editable: true
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    validator: DoubleValidator {
                        bottom: duration.from / 1000
                        top: duration.to / 1000
                        decimals: 3
                        notation: DoubleValidator.StandardNotation
                        locale: duration.locale.name
                    }
                    // Keep milliseconds on disk, but edit localized seconds.
                    // Four significant digits preserve every allowed millisecond.
                    textFromValue: function(value, locale) {
                        return Number(value / 1000).toLocaleString(locale, "g", 4)
                    }
                    valueFromText: function(text, locale) {
                        return Math.round(Number.fromLocaleString(locale, text) * 1000)
                    }
                    value: EasimeSettings.indicatorMs
                    onValueModified: {
                        EasimeSettings.applyIndicatorMs(value)
                        value = Qt.binding(() => EasimeSettings.indicatorMs)
                    }
                }
                QQC2.Label {
                    text: Tr.t("초")
                }
            }

            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: Tr.t("입력창을 누르거나 창을 옮길 때는 표시하지 않고, 사용자가 언어를 바꿀 때만 잠깐 나타납니다.")
            }
        }
    }
}
