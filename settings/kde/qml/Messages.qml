// What went wrong with the settings file, and what the last action did.
//
// Good news ("saved") is a short message at the bottom of the window, so it
// never moves the controls the user is working with; only problems take room
// on the page.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

ColumnLayout {
    id: messages

    readonly property var failures: ["unreadable", "no_home", "write_failed", "invalid", "activation_changed"]

    function goodNews(kind, detail) {
        switch (kind) {
        case "saved":
            return Tr.t("저장했습니다. 실행 중인 easime에 곧 반영됩니다.")
        case "reloaded":
            return Tr.t("설정 파일을 다시 읽었습니다.")
        case "activated":
            return Tr.t("easime를 입력기로 선택했습니다. 다시 로그인하면 사용할 수 있습니다.")
        case "deactivated":
            return Tr.t("이전 입력기로 되돌렸습니다. 다시 로그인하면 적용됩니다.")
        case "reset":
            return detail !== ""
                ? Tr.t("기본값으로 되돌렸습니다. 이전 파일은 %1에 보관했습니다.").arg(detail)
                : Tr.t("기본값으로 되돌렸습니다.")
        }
        return ""
    }

    // Here, not in the Connections below: the Window attached property only
    // works on an Item.
    function announce() {
        var text = goodNews(EasimeSettings.resultKind, EasimeSettings.resultDetail)
        var window = Window.window
        if (text !== "" && window) {
            window.showPassiveNotification(text, EasimeSettings.resultKind === "reset" ? 8000 : 3000)
        }
    }

    spacing: Kirigami.Units.smallSpacing

    Connections {
        target: EasimeSettings

        function onResultKindChanged() {
            messages.announce()
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: EasimeSettings.problemKind !== ""
        type: Kirigami.MessageType.Error
        text: {
            switch (EasimeSettings.problemKind) {
            case "unreadable":
                return Tr.t("설정 파일을 읽지 못해 기본값을 보여 드리고 있습니다. 파일을 직접 고친 뒤 '다시 읽기'를 누르거나, '일반'에서 '기본값으로 되돌리기'를 눌러 새로 시작하세요. 읽을 수 없는 동안에는 파일을 지우지 않도록 변경을 저장하지 않습니다.")
                    + "\n" + EasimeSettings.problemDetail
            case "no_home":
                return Tr.t("홈 폴더를 찾지 못해 설정을 저장할 수 없습니다.")
            }
            return ""
        }
        actions: [
            Kirigami.Action {
                text: Tr.t("다시 읽기")
                icon.name: "view-refresh"
                onTriggered: EasimeSettings.reload()
            }
        ]
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: messages.failures.indexOf(EasimeSettings.resultKind) >= 0
        type: Kirigami.MessageType.Warning
        text: {
            switch (EasimeSettings.resultKind) {
            case "unreadable":
                return Tr.t("설정 파일을 읽을 수 없어 저장하지 않았습니다.")
            case "no_home":
                return Tr.t("홈 폴더를 찾지 못해 저장하지 못했습니다.")
            case "write_failed":
                return Tr.t("저장하지 못했습니다. 변경하기 전 값을 그대로 보여 드립니다.") + "\n" + EasimeSettings.resultDetail
            case "invalid":
                return Tr.t("알 수 없는 값이라 적용하지 않았습니다.")
            case "activation_changed":
                return Tr.t("그 사이 다른 입력기가 선택되어 있어 되돌리지 않았습니다.")
            }
            return ""
        }
    }
}
