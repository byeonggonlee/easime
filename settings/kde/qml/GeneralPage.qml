import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    id: page

    title: Tr.t("일반")

    // The Window attached property only works on an Item, so not inside the action.
    function openLanguages() {
        var window = Window.window
        if (window) {
            window.openLanguages()
        }
    }

    function folderOf(path) {
        return path.replace(/[\/\\][^\/\\]*$/, "")
    }

    // The same two steps a person takes (the button opens the question, "OK"
    // answers it); the CI uses them because it has no mouse.
    function openActivationDialog() { confirmActivation.open() }
    function acceptActivationDialog() { confirmActivation.accept() }
    function openUndoDialog() { confirmUndo.open() }
    function acceptUndoDialog() { confirmUndo.accept() }

    readonly property string currentInputMethod: EasimeSettings.activationCurrent !== ""
        ? EasimeSettings.activationCurrent : Tr.t("(없음)")
    readonly property string undoInputMethod: EasimeSettings.activationUndoName !== ""
        ? EasimeSettings.activationUndoName : Tr.t("(없음)")

    // Opening the page is when the dictionary is looked for: it opens the files.
    Component.onCompleted: EasimeSettings.refreshDictionary()

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
        }

        // Installing the package does not select easime as the input method.
        // Choosing it changes a KDE setting, so it asks first and can be undone.
        Kirigami.InlineMessage {
            objectName: "activation-card"
            Layout.fillWidth: true
            visible: EasimeSettings.activationState !== "unsupported"
            type: {
                switch (EasimeSettings.activationState) {
                case "selected":
                case "running":
                    return Kirigami.MessageType.Positive
                case "not_installed":
                    return Kirigami.MessageType.Warning
                }
                return Kirigami.MessageType.Information
            }
            text: {
                switch (EasimeSettings.activationState) {
                case "other":
                    return Tr.t("easime가 아직 입력기로 선택되지 않았습니다. 선택하면 다음에 로그인할 때부터 easime로 입력합니다.")
                case "selected":
                    return Tr.t("easime를 입력기로 선택했습니다. 다시 로그인하면 사용할 수 있습니다.")
                case "running":
                    return Tr.t("easime가 입력기로 선택되어 실행 중입니다. 글을 쓰는 곳에서 입력해 보세요. 입력되지 않으면 로그 파일을 확인하세요.")
                case "not_installed":
                    return Tr.t("easime의 입력기 파일(easime.desktop)을 찾지 못해 입력기로 선택할 수 없습니다. easime가 제대로 설치되었는지 확인해 주세요.")
                }
                return ""
            }
            actions: [
                Kirigami.Action {
                    objectName: "activate"
                    text: Tr.t("입력기로 사용하기…")
                    icon.name: "dialog-ok-apply"
                    visible: EasimeSettings.activationState === "other"
                    onTriggered: page.openActivationDialog()
                },
                Kirigami.Action {
                    objectName: "undo-activation"
                    text: Tr.t("이전 입력기로 되돌리기…")
                    icon.name: "edit-undo"
                    visible: EasimeSettings.activationCanUndo
                    onTriggered: page.openUndoDialog()
                }
            ]
        }

        // Every input language off is a valid choice, but someone who installed
        // easime on a system whose language is not Korean or Japanese would take
        // English-only typing for a fault: say so and lead to the page.
        Kirigami.InlineMessage {
            objectName: "no-language-card"
            Layout.fillWidth: true
            visible: EasimeSettings.languages === ""
            type: Kirigami.MessageType.Information
            text: Tr.t("지금은 영어로만 입력됩니다. 한국어나 日本語로도 입력하려면 언어 화면에서 켜 주세요.")
            actions: [
                Kirigami.Action {
                    objectName: "open-languages"
                    text: Tr.t("언어 화면 열기")
                    icon.name: "preferences-desktop-locale"
                    onTriggered: page.openLanguages()
                }
            ]
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: Tr.t("바꾼 내용은 바로 설정 파일에 저장되고, 실행 중인 easime가 곧 다시 읽습니다. 다시 시작할 필요는 없습니다.")
        }

        // Translations are drafts until a native speaker has reviewed them.
        QQC2.Label {
            Layout.fillWidth: true
            visible: Tr.isDraft()
            wrapMode: Text.WordWrap
            opacity: 0.7
            text: Tr.t("이 설정 창의 번역은 초안이며 원어민 검토를 거치지 않았습니다.")
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            RowLayout {
                Kirigami.FormData.label: Tr.t("설정 파일:")
                QQC2.Label {
                    text: EasimeSettings.configPath !== "" ? EasimeSettings.configPath : Tr.t("(없음)")
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 22
                }
                QQC2.Button {
                    text: Tr.t("폴더 열기")
                    icon.name: "folder-open"
                    enabled: EasimeSettings.configPath !== ""
                    onClicked: Qt.openUrlExternally("file://" + page.folderOf(EasimeSettings.configPath))
                }
            }

            RowLayout {
                Kirigami.FormData.label: Tr.t("로그 파일:")
                QQC2.Label {
                    text: EasimeSettings.logPath !== "" ? EasimeSettings.logPath : Tr.t("(없음)")
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 22
                }
                QQC2.Button {
                    text: Tr.t("폴더 열기")
                    icon.name: "folder-open"
                    enabled: EasimeSettings.logPath !== ""
                    onClicked: Qt.openUrlExternally("file://" + page.folderOf(EasimeSettings.logPath))
                }
            }

            RowLayout {
                Kirigami.FormData.label: Tr.t("직접 고쳤다면:")
                QQC2.Button {
                    text: Tr.t("다시 읽기")
                    icon.name: "view-refresh"
                    onClicked: EasimeSettings.reload()
                }
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: Tr.t("지원 상태")
            }

            QQC2.Label {
                Kirigami.FormData.label: Tr.t("한국어:")
                text: Tr.t("두벌식 입력 · 조합 중 한 글자 한자 변환(시험 중)")
            }
            QQC2.Label {
                Kirigami.FormData.label: Tr.t("日本語:")
                text: Tr.t("로마자 → 가나 입력 · 단어 단위 한자 변환(시험 중)")
            }
            QQC2.Label {
                Kirigami.FormData.label: Tr.t("中文:")
                text: Tr.t("준비 중")
            }
            QQC2.Label {
                objectName: "dictionary-status"
                Kirigami.FormData.label: Tr.t("일본어 사전:")
                visible: EasimeSettings.dictionaryState !== "unknown"
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                wrapMode: Text.WordWrap
                text: {
                    switch (EasimeSettings.dictionaryState) {
                    case "found":
                        return Tr.t("있음 (파일 %1개). 단어 단위로 한자를 고를 수 있습니다.").arg(EasimeSettings.dictionaryDetail)
                    case "missing":
                        return Tr.t("찾지 못했습니다. 일본어를 켜도 가나만 후보로 나옵니다. easime 패키지를 다시 설치하거나 사전 파일을 아래 폴더 중 한 곳에 두세요:\n%1").arg(EasimeSettings.dictionaryDetail)
                    case "unreadable":
                        return Tr.t("읽지 못한 사전 파일이 %1개 있습니다. 나머지로 변환하며, 자세한 이유는 로그 파일에 있습니다.").arg(EasimeSettings.dictionaryDetail)
                    }
                    return ""
                }
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: Tr.t("처음부터 다시")
            }

            QQC2.Button {
                Kirigami.FormData.label: Tr.t("설정 초기화:")
                text: Tr.t("easime 설정을 기본값으로 되돌리기")
                icon.name: "edit-reset"
                onClicked: confirmReset.open()
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmActivation

        title: Tr.t("easime를 입력기로 사용할까요?")
        subtitle: Tr.t("KDE의 가상 키보드 설정을 바꿉니다.\n파일: %1\n지금 선택된 입력기: %2 → easime\n바꾼 뒤 다시 로그인하면 적용되며, 이 화면에서 언제든 되돌릴 수 있습니다.")
            .arg(EasimeSettings.activationFile).arg(page.currentInputMethod)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: EasimeSettings.applyActivation()
    }

    Kirigami.PromptDialog {
        id: confirmUndo

        title: Tr.t("이전 입력기로 되돌릴까요?")
        subtitle: Tr.t("가상 키보드 설정을 easime를 고르기 전의 값(%1)으로 되돌립니다. 다시 로그인하면 적용됩니다.")
            .arg(page.undoInputMethod)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: EasimeSettings.undoActivation()
    }

    Kirigami.PromptDialog {
        id: confirmReset

        title: Tr.t("기본값으로 되돌릴까요?")
        subtitle: Tr.t("easime의 설정이 모두 기본값으로 바뀝니다. 지금 설정 파일은 config.toml.bak으로 보관되며, 입력기 선택이나 다른 프로그램의 설정은 바뀌지 않습니다.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: EasimeSettings.resetToDefaults()
    }
}
