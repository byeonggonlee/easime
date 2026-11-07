import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ScrollablePage {
    id: page

    title: Tr.t("일반")

    function folderOf(path) {
        return path.replace(/[\/\\][^\/\\]*$/, "")
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Messages {
            Layout.fillWidth: true
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
        id: confirmReset

        title: Tr.t("기본값으로 되돌릴까요?")
        subtitle: Tr.t("easime의 설정이 모두 기본값으로 바뀝니다. 지금 설정 파일은 config.toml.bak으로 보관되며, 입력기 선택이나 다른 프로그램의 설정은 바뀌지 않습니다.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: EasimeSettings.resetToDefaults()
    }
}
