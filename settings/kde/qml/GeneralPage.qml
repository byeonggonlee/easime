import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.easime.settings

Kirigami.ScrollablePage {
    id: page

    title: qsTr("일반")

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
            text: qsTr("바꾼 내용은 바로 설정 파일에 저장되고, 실행 중인 easime가 곧 다시 읽습니다. 다시 시작할 필요는 없습니다.")
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            RowLayout {
                Kirigami.FormData.label: qsTr("설정 파일:")
                QQC2.Label {
                    text: EasimeSettings.configPath !== "" ? EasimeSettings.configPath : qsTr("(없음)")
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 22
                }
                QQC2.Button {
                    text: qsTr("폴더 열기")
                    icon.name: "folder-open"
                    enabled: EasimeSettings.configPath !== ""
                    onClicked: Qt.openUrlExternally("file://" + page.folderOf(EasimeSettings.configPath))
                }
            }

            RowLayout {
                Kirigami.FormData.label: qsTr("로그 파일:")
                QQC2.Label {
                    text: EasimeSettings.logPath !== "" ? EasimeSettings.logPath : qsTr("(없음)")
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 22
                }
                QQC2.Button {
                    text: qsTr("폴더 열기")
                    icon.name: "folder-open"
                    enabled: EasimeSettings.logPath !== ""
                    onClicked: Qt.openUrlExternally("file://" + page.folderOf(EasimeSettings.logPath))
                }
            }

            RowLayout {
                Kirigami.FormData.label: qsTr("직접 고쳤다면:")
                QQC2.Button {
                    text: qsTr("다시 읽기")
                    icon.name: "view-refresh"
                    onClicked: EasimeSettings.reload()
                }
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("지원 상태")
            }

            QQC2.Label {
                Kirigami.FormData.label: qsTr("한국어:")
                text: qsTr("두벌식 입력 · 조합 중 한 글자 한자 변환(시험 중)")
            }
            QQC2.Label {
                Kirigami.FormData.label: qsTr("日本語:")
                text: qsTr("로마자 → 가나 입력 · 단어 단위 한자 변환(시험 중)")
            }
            QQC2.Label {
                Kirigami.FormData.label: qsTr("中文:")
                text: qsTr("준비 중")
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("처음부터 다시")
            }

            QQC2.Button {
                Kirigami.FormData.label: qsTr("설정 초기화:")
                text: qsTr("easime 설정을 기본값으로 되돌리기")
                icon.name: "edit-reset"
                onClicked: confirmReset.open()
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmReset

        title: qsTr("기본값으로 되돌릴까요?")
        subtitle: qsTr("easime의 설정이 모두 기본값으로 바뀝니다. 지금 설정 파일은 config.toml.bak으로 보관되며, 입력기 선택이나 다른 프로그램의 설정은 바뀌지 않습니다.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: EasimeSettings.resetToDefaults()
    }
}
