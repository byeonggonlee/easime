import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.easime.settings

Kirigami.ApplicationWindow {
    id: root

    title: qsTr("easime 설정")
    width: Kirigami.Units.gridUnit * 46
    height: Kirigami.Units.gridUnit * 30
    minimumWidth: Kirigami.Units.gridUnit * 30
    minimumHeight: Kirigami.Units.gridUnit * 20

    // `--screenshots=DIR` saves every page as DIR/<name>.png and quits. The
    // CI uses it to check that all pages load and to keep pictures of them.
    readonly property string screenshotDir: {
        var args = Qt.application.arguments
        for (var i = 0; i < args.length; ++i) {
            if (args[i].indexOf("--screenshots=") === 0) {
                return args[i].substring("--screenshots=".length)
            }
        }
        return ""
    }

    property int current: -1
    readonly property var entries: [
        { name: "general", title: qsTr("일반"), icon: "configure", page: generalPage },
        { name: "languages", title: qsTr("언어"), icon: "preferences-desktop-locale", page: languagesPage },
        { name: "shortcuts", title: qsTr("단축키"), icon: "preferences-desktop-keyboard-shortcut", page: shortcutsPage },
        { name: "korean", title: qsTr("한국어"), icon: "input-keyboard", page: koreanPage },
        { name: "appearance", title: qsTr("모양"), icon: "preferences-desktop-theme", page: appearancePage }
    ]

    function show(index) {
        if (index === current) {
            return
        }
        current = index
        pageStack.clear()
        pageStack.push(entries[index].page)
    }

    globalDrawer: Kirigami.GlobalDrawer {
        modal: false
        collapsible: false
        actions: [
            Kirigami.Action {
                text: root.entries[0].title
                icon.name: root.entries[0].icon
                checkable: true
                checked: root.current === 0
                onTriggered: root.show(0)
            },
            Kirigami.Action {
                text: root.entries[1].title
                icon.name: root.entries[1].icon
                checkable: true
                checked: root.current === 1
                onTriggered: root.show(1)
            },
            Kirigami.Action {
                text: root.entries[2].title
                icon.name: root.entries[2].icon
                checkable: true
                checked: root.current === 2
                onTriggered: root.show(2)
            },
            Kirigami.Action {
                text: root.entries[3].title
                icon.name: root.entries[3].icon
                checkable: true
                checked: root.current === 3
                onTriggered: root.show(3)
            },
            Kirigami.Action {
                text: root.entries[4].title
                icon.name: root.entries[4].icon
                checkable: true
                checked: root.current === 4
                onTriggered: root.show(4)
            }
        ]
    }

    Component { id: generalPage; GeneralPage {} }
    Component { id: languagesPage; LanguagesPage {} }
    Component { id: shortcutsPage; ShortcutsPage {} }
    Component { id: koreanPage; KoreanPage {} }
    Component { id: appearancePage; AppearancePage {} }

    Component.onCompleted: {
        show(0)
        console.info("SETTINGS-WINDOW-LOADED pages=" + entries.length)
    }

    Timer {
        id: tour

        property int step: 0

        interval: 900
        repeat: true
        running: root.screenshotDir !== ""
        onTriggered: {
            if (step > 0) {
                var name = root.entries[step - 1].name
                var path = root.screenshotDir + "/" + name + ".png"
                var requested = root.contentItem.grabToImage(function (result) {
                    console.info("SCREENSHOT " + name + " " + result.saveToFile(path))
                })
                console.info("GRAB-REQUESTED " + name + " " + requested
                    + " size=" + root.contentItem.width + "x" + root.contentItem.height
                    + " visible=" + root.visible)
            }
            if (step < root.entries.length) {
                root.show(step)
                step += 1
            } else {
                stop()
                quitLater.start()
            }
        }
    }

    Timer {
        id: quitLater

        interval: 1200
        onTriggered: Qt.quit()
    }
}
