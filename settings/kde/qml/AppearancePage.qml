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

                // A native text field exposes the same seconds to assistive
                // technology as to sighted users; an integer SpinBox would
                // expose milliseconds through its numeric accessibility API.
                QQC2.TextField {
                    id: duration
                    objectName: "indicator-duration"
                    Accessible.name: Tr.t("표시 시간 (초)")
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                    selectByMouse: true
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    readonly property var locale: Qt.locale()
                    readonly property int value: EasimeSettings.indicatorMs
                    validator: DoubleValidator {
                        bottom: 0.1
                        top: 10
                        decimals: 3
                        notation: DoubleValidator.StandardNotation
                        locale: duration.locale.name
                    }
                    function textFromValue(value, locale) {
                        return Number(value / 1000).toLocaleString(locale, "g", 4)
                    }
                    function valueFromText(text, locale) {
                        return Math.round(Number.fromLocaleString(locale, text) * 1000)
                    }
                    function save(milliseconds) {
                        EasimeSettings.applyIndicatorMs(milliseconds)
                        text = Qt.binding(() => textFromValue(value, locale))
                    }
                    function step(amount) {
                        var base = acceptableInput ? valueFromText(text, locale) : value
                        save(Math.max(100, Math.min(10000, base + amount)))
                    }
                    text: textFromValue(value, locale)
                    onEditingFinished: {
                        if (acceptableInput) save(valueFromText(text, locale))
                        else text = Qt.binding(() => textFromValue(value, locale))
                    }
                    // Reject incomplete edits on blur as well as on Enter.
                    onActiveFocusChanged: {
                        if (!activeFocus && !acceptableInput)
                            text = Qt.binding(() => textFromValue(value, locale))
                    }
                    Keys.onUpPressed: step(100)
                    Keys.onDownPressed: step(-100)
                }
                QQC2.Label {
                    text: Tr.t("초")
                }
                QQC2.ToolButton {
                    objectName: "duration-decrease"
                    text: "−"
                    Accessible.name: Tr.t("표시 시간 줄이기")
                    enabled: duration.value > 100
                    onClicked: duration.step(-100)
                    QQC2.ToolTip.visible: hovered || activeFocus
                    QQC2.ToolTip.text: Accessible.name
                }
                QQC2.ToolButton {
                    objectName: "duration-increase"
                    text: "+"
                    Accessible.name: Tr.t("표시 시간 늘리기")
                    enabled: duration.value < 10000
                    onClicked: duration.step(100)
                    QQC2.ToolTip.visible: hovered || activeFocus
                    QQC2.ToolTip.text: Accessible.name
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
