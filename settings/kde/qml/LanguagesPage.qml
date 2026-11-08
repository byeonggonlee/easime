import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    id: page

    title: Tr.t("언어")

    readonly property var enabledCodes: EasimeSettings.languages === ""
        ? []
        : EasimeSettings.languages.split(",")
    readonly property var names: ({ korean: "한국어", japanese: "日本語" })
    readonly property var orderedCodes: {
        var codes = enabledCodes.slice()
        for (var code of ["korean", "japanese"]) {
            if (codes.indexOf(code) < 0) {
                codes.push(code)
            }
        }
        return codes
    }
    // Reorder actual children as well as pixels, keeping Tab order visual.
    // Rebuilt delegates restore focus to the same language after the update.
    function restoreFocus(code, prefix) {
        Qt.callLater(function() {
            var row = languageRows.itemAt(page.orderedCodes.indexOf(code))
            if (!row) return
            var fallback = null
            for (var child of row.children) {
                if (child.objectName === "switch-" + code) fallback = child
                if (child.objectName === prefix + code && child.enabled) {
                    child.forceActiveFocus(Qt.TabFocusReason)
                    return
                }
            }
            if (fallback) fallback.forceActiveFocus(Qt.TabFocusReason)
        })
    }

    readonly property string cycle: {
        var parts = [Tr.t("영어")]
        for (var i = 0; i < enabledCodes.length; ++i) {
            parts.push(names[enabledCodes[i]])
        }
        parts.push(Tr.t("영어"))
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
            text: Tr.t("쓰는 언어만 켜 두세요. 어느 언어도 꼭 켜야 하는 것은 아닙니다. 전환 키를 누를 때마다 아래 순서대로 바뀌며, 열려 있는 모든 창에 똑같이 적용됩니다.")
        }

        // What decides the languages that are on now: not yet the user.
        QQC2.Label {
            Layout.fillWidth: true
            visible: EasimeSettings.languagesFollowSystem
            wrapMode: Text.WordWrap
            opacity: 0.7
            text: Tr.t("아직 직접 고르지 않아 시스템 언어에 맞춰져 있습니다. 직접 바꾸면 그 선택이 저장됩니다.")
        }

        Repeater {
            id: languageRows
            model: page.orderedCodes

            delegate: RowLayout {
                id: row
                required property string modelData
                readonly property int position: page.enabledCodes.indexOf(modelData)
                readonly property bool on: position >= 0
                Layout.fillWidth: true

                QQC2.Switch {
                    id: languageSwitch
                    objectName: "switch-" + row.modelData
                    text: page.names[row.modelData]
                    checked: row.on
                    onToggled: {
                        var code = row.modelData
                        var requested = checked
                        checked = Qt.binding(() => row.on)
                        page.restoreFocus(code, "switch-")
                        EasimeSettings.setLanguageEnabled(code, requested)
                    }
                }
                Item { Layout.fillWidth: true }
                QQC2.ToolButton {
                    objectName: "move-up-" + row.modelData
                    icon.name: "go-up"
                    text: Tr.t("%1: 전환 순서 앞으로").arg(page.names[row.modelData])
                    display: QQC2.AbstractButton.IconOnly
                    enabled: row.on && row.position > 0
                    onClicked: {
                        var code = row.modelData
                        page.restoreFocus(code, "move-up-")
                        EasimeSettings.moveLanguage(code, true)
                    }
                    QQC2.ToolTip.visible: hovered || activeFocus
                    QQC2.ToolTip.text: text
                }
                QQC2.ToolButton {
                    objectName: "move-down-" + row.modelData
                    icon.name: "go-down"
                    text: Tr.t("%1: 전환 순서 뒤로").arg(page.names[row.modelData])
                    display: QQC2.AbstractButton.IconOnly
                    enabled: row.on && row.position < page.enabledCodes.length - 1
                    onClicked: {
                        var code = row.modelData
                        page.restoreFocus(code, "move-down-")
                        EasimeSettings.moveLanguage(code, false)
                    }
                    QQC2.ToolTip.visible: hovered || activeFocus
                    QQC2.ToolTip.text: text
                }
            }
        }

        // Simplified and Traditional Chinese are separate entries, as on
        // Windows and macOS. Neither has an engine yet.
        Repeater {
            model: ["简体中文", "繁體中文"]

            delegate: RowLayout {
                required property string modelData

                Layout.fillWidth: true
                enabled: false

                QQC2.Switch {
                    text: modelData
                    checked: false
                }
                QQC2.Label {
                    text: Tr.t("준비 중")
                }
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
                ? Tr.t("전환 순서: %1").arg(page.cycle)
                : Tr.t("켜진 언어가 없어 영어로만 입력합니다.")
        }
    }
}
