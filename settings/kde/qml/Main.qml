import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.easime.settings
import "tr.js" as Tr

Kirigami.ApplicationWindow {
    id: root

    title: Tr.t("easime 설정")
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
        { name: "general", file: "GeneralPage.qml", title: Tr.t("일반"), icon: "configure" },
        { name: "languages", file: "LanguagesPage.qml", title: Tr.t("언어"), icon: "preferences-desktop-locale" },
        { name: "shortcuts", file: "ShortcutsPage.qml", title: Tr.t("단축키"), icon: "preferences-desktop-keyboard-shortcut" },
        { name: "korean", file: "KoreanPage.qml", title: Tr.t("한국어"), icon: "input-keyboard" },
        { name: "appearance", file: "AppearancePage.qml", title: Tr.t("모양"), icon: "preferences-desktop-theme" }
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
        if (name === "general") {
            if (EasimeSettings.languages === "" && find(page, "dictionary-status").visible) {
                console.error("UX-CHECK-FAILED dictionary notice shown for English-only typing")
            }
            // Select easime as the input method like a person would: the button
            // asks, "OK" answers. Only where there is something to select.
            console.info("INTERACTION activation-before state=" + EasimeSettings.activationState)
            if (EasimeSettings.activationState === "other") {
                page.openActivationDialog()
                page.acceptActivationDialog()
                console.info("INTERACTION activation-after state=" + EasimeSettings.activationState
                    + " undo=" + EasimeSettings.activationCanUndo + " result=" + EasimeSettings.resultKind)
            }
        } else if (name === "general-undo") {
            if (EasimeSettings.languages === "japanese" && !find(page, "dictionary-status").visible) {
                console.error("UX-CHECK-FAILED dictionary notice hidden for Japanese typing")
            }
            // The end of the tour puts the selection back, also like a person would.
            if (EasimeSettings.activationCanUndo) {
                page.openUndoDialog()
                page.acceptUndoDialog()
                console.info("INTERACTION activation-undone state=" + EasimeSettings.activationState
                    + " undo=" + EasimeSettings.activationCanUndo + " result=" + EasimeSettings.resultKind)
            }
        } else if (name === "languages") {
            var japanese = find(page, "switch-japanese")
            japanese.click()
            console.info("INTERACTION japanese-on languages=" + EasimeSettings.languages
                + " switch=" + japanese.checked + " result=" + EasimeSettings.resultKind)
            var after = []
            describe(page, page, 0, after)
            console.info("STRUCTURE-AFTER languages\n" + after.join("\n"))
        } else if (name === "languages-enable-korean") {
            find(page, "switch-korean").click()
        } else if (name === "languages-reorder") {
            find(page, "move-up-korean").click()
        } else if (name === "languages-verify") {
            var korean = find(page, "switch-korean")
            var japanese = find(page, "switch-japanese")
            if (EasimeSettings.languages !== "korean,japanese"
                    || korean.mapToItem(page, 0, 0).y >= japanese.mapToItem(page, 0, 0).y) {
                console.error("UX-CHECK-FAILED language rows do not follow the saved order")
            }
            console.info("UX-CHECK language-order=" + EasimeSettings.languages)
            korean.click()
        } else if (name === "shortcuts") {
            var hangul = find(page, "choice-hangul")
            hangul.click()
            console.info("INTERACTION toggle-key=" + EasimeSettings.toggleKey
                + " hangul=" + hangul.checked + " both=" + find(page, "choice-both").checked)
        } else if (name === "appearance") {
            var duration = find(page, "indicator-duration")
            for (var localeName of ["en_US", "de_DE", "ko_KR"]) {
                var locale = Qt.locale(localeName)
                for (var ms of [100, 1000, 1250, 9999, 10000]) {
                    var display = duration.textFromValue(ms, locale)
                    if (duration.valueFromText(display, locale) !== ms
                            || Number.fromLocaleString(locale, display) !== ms / 1000) {
                        console.error("UX-CHECK-FAILED seconds round trip " + localeName + " " + ms)
                    }
                }
            }
            duration.increase()
            duration.valueModified()
            if (EasimeSettings.indicatorMs !== 1100) {
                console.error("UX-CHECK-FAILED duration was not saved")
            }
            EasimeSettings.applyIndicatorMs(1250)
            if (duration.value !== 1250) {
                console.error("UX-CHECK-FAILED duration binding was lost after editing")
            }
            EasimeSettings.applyIndicatorMs(1000)
            console.info("UX-CHECK duration-seconds")
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
        EasimeSettings.clearResult()
        pageStack.clear()
        pageStack.push(Qt.resolvedUrl(entries[index].file))
    }

    // For a page that leads to the Languages page (index 1).
    function openLanguages() {
        show(1)
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
        console.info("INPUT-LANGUAGES languages=" + EasimeSettings.languages
            + " follow-system=" + EasimeSettings.languagesFollowSystem)
    }

    // What the screenshot tour does, one action per tick: show a page, take its
    // picture, use its controls, take the picture again. A picture is taken
    // while its page is still on screen and never right before the next page
    // replaces it (the grab is rendered on the next frame).
    function planTour() {
        var plan = []
        for (var i = 0; i < entries.length; ++i) {
            var name = entries[i].name
            plan.push({ kind: "show", index: i })
            if (i === 0) {
                plan.push({ kind: "sidebar" })
            }
            plan.push({ kind: "capture", name: name, describe: true })
            if (name === "general" || name === "languages" || name === "shortcuts" || name === "appearance") {
                plan.push({ kind: "interact", name: name })
                if (name === "languages") {
                    plan.push({ kind: "interact", name: "languages-enable-korean" })
                    plan.push({ kind: "interact", name: "languages-reorder" })
                    plan.push({ kind: "interact", name: "languages-verify" })
                }
                plan.push({ kind: "capture", name: name + "-after", describe: false })
            }
        }
        // Back to the first page to undo what the tour selected there.
        plan.push({ kind: "show", index: 0 })
        plan.push({ kind: "interact", name: "general-undo" })
        return plan
    }

    readonly property var tourPlan: planTour()

    Timer {
        id: tour

        property int next: 0

        interval: 900
        repeat: true
        running: root.screenshotDir !== ""
        onTriggered: {
            var action = root.tourPlan[next]
            next += 1
            if (action === undefined) {
                stop()
                quitLater.start()
                return
            }
            var page = root.pageStack.currentItem
            if (action.kind === "show") {
                root.show(action.index)
            } else if (action.kind === "sidebar") {
                var side = root.globalDrawer.contentItem
                var rows = []
                root.describe(side, side, 0, rows)
                console.info("STRUCTURE sidebar (" + side.width + "x" + side.height + ")\n" + rows.join("\n"))
            } else if (action.kind === "capture") {
                if (action.describe) {
                    var lines = []
                    root.describe(page, page, 0, lines)
                    console.info("STRUCTURE " + action.name + " (" + page.width + "x" + page.height + ")\n"
                        + lines.join("\n"))
                }
                var path = root.screenshotDir + "/" + action.name + ".png"
                var requested = root.pageStack.grabToImage(function (result) {
                    console.info("SCREENSHOT " + action.name + " " + result.saveToFile(path))
                })
                console.info("GRAB-REQUESTED " + action.name + " " + requested)
            } else if (action.kind === "interact") {
                root.interact(action.name, page)
            }
        }
    }

    Timer {
        id: quitLater

        interval: 1200
        onTriggered: Qt.quit()
    }
}
