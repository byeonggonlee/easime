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
        { name: "general", file: "GeneralPage.qml", title: qsTr("일반"), icon: "configure" },
        { name: "languages", file: "LanguagesPage.qml", title: qsTr("언어"), icon: "preferences-desktop-locale" },
        { name: "shortcuts", file: "ShortcutsPage.qml", title: qsTr("단축키"), icon: "preferences-desktop-keyboard-shortcut" },
        { name: "korean", file: "KoreanPage.qml", title: qsTr("한국어"), icon: "input-keyboard" },
        { name: "appearance", file: "AppearancePage.qml", title: qsTr("모양"), icon: "preferences-desktop-theme" }
    ]

    // One line per visible item that has text: what it says, where it is on
    // the page and how big, and whether it is checked or disabled. The CI
    // prints this so a page can be read without seeing it.
    function describe(item, page, depth, lines) {
        if (!item || !item.visible || depth > 16) {
            return
        }
        if (typeof item.text === "string" && item.text !== "") {
            var at = item.mapToItem(page, 0, 0)
            var kind = item.toString().split("_QML")[0].split("(")[0]
            lines.push("  ".repeat(Math.min(depth, 8)) + kind + " " + JSON.stringify(item.text.substring(0, 70))
                + " @" + Math.round(at.x) + "," + Math.round(at.y)
                + " " + Math.round(item.width) + "x" + Math.round(item.height)
                + (item.checked === true ? " checked" : "")
                + (item.enabled === false ? " disabled" : ""))
        }
        var kids = item.children
        for (var i = 0; i < kids.length; ++i) {
            describe(kids[i], page, depth + 1, lines)
        }
    }

    function find(item, name) {
        if (!item) {
            return null
        }
        if (item.objectName === name) {
            return item
        }
        var kids = item.children
        for (var i = 0; i < kids.length; ++i) {
            var hit = find(kids[i], name)
            if (hit) {
                return hit
            }
        }
        return null
    }

    // Use a page like a person would, so the CI sees the whole path from a
    // control to the settings file. click() is what a user does; toggle()
    // only flips `checked` and never emits toggled(), which the pages listen to.
    // The pictures that follow show the result.
    function interact(name, page) {
        if (name === "languages") {
            var japanese = find(page, "switch-japanese")
            japanese.click()
            console.info("INTERACTION japanese-on languages=" + EasimeSettings.languages
                + " switch=" + japanese.checked + " result=" + EasimeSettings.resultKind)
            var after = []
            describe(page, page, 0, after)
            console.info("STRUCTURE-AFTER languages\n" + after.join("\n"))
        } else if (name === "shortcuts") {
            var hangul = find(page, "choice-hangul")
            hangul.click()
            console.info("INTERACTION toggle-key=" + EasimeSettings.toggleKey
                + " hangul=" + hangul.checked + " both=" + find(page, "choice-both").checked)
        } else if (name === "appearance") {
            var box = find(page, "box")
            box.click()
            console.info("INTERACTION indicator-box=" + EasimeSettings.indicatorBox
                + " checkbox=" + box.checked)
        }
    }

    function show(index) {
        if (index === current) {
            return
        }
        current = index
        pageStack.clear()
        pageStack.push(Qt.resolvedUrl(entries[index].file))
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
                var lines = []
                describe(root.pageStack.currentItem, root.pageStack.currentItem, 0, lines)
                console.info("STRUCTURE " + name + " (" + root.pageStack.currentItem.width + "x"
                    + root.pageStack.currentItem.height + ")\n" + lines.join("\n"))
                interact(name, root.pageStack.currentItem)
                var path = root.screenshotDir + "/" + name + ".png"
                var requested = root.pageStack.grabToImage(function (result) {
                    console.info("SCREENSHOT " + name + " " + result.saveToFile(path))
                })
                console.info("GRAB-REQUESTED " + name + " " + requested
                    + " size=" + root.pageStack.width + "x" + root.pageStack.height
                    + " drawer=" + root.globalDrawer.visible + "/" + root.globalDrawer.width)
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
