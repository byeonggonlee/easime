// Run with: qmltestrunner -input settings/kde/tests/tst_tr.qml
import QtQuick
import QtTest
import "../qml/tr.js" as Tr

TestCase {
    name: "Tr"

    function test_pick_known_languages() {
        compare(Tr.pick("ko"), "ko")
        compare(Tr.pick("ko_KR"), "ko")
        compare(Tr.pick("ko-KR"), "ko")
        compare(Tr.pick("ja"), "ja")
        compare(Tr.pick("ja_JP.UTF-8"), "ja")
        compare(Tr.pick("en"), "en")
        compare(Tr.pick("en_US"), "en")
    }

    function test_chinese_script_follows_the_region() {
        compare(Tr.pick("zh_TW"), "zh-Hant")
        compare(Tr.pick("zh-HK"), "zh-Hant")
        compare(Tr.pick("zh-MO"), "zh-Hant")
        compare(Tr.pick("zh-Hant"), "zh-Hant")
        compare(Tr.pick("zh-Hant-TW"), "zh-Hant")
        compare(Tr.pick("zh_CN"), "zh-Hans")
        compare(Tr.pick("zh-SG"), "zh-Hans")
        compare(Tr.pick("zh-Hans"), "zh-Hans")
        compare(Tr.pick("zh"), "zh-Hans")
    }

    function test_other_tags_are_not_ours() {
        compare(Tr.pick("fr_FR"), "")
        compare(Tr.pick("kok"), "")
        compare(Tr.pick("jam"), "")
        compare(Tr.pick("C"), "")
        compare(Tr.pick(""), "")
    }

    function test_detect_prefers_the_request_then_the_system() {
        compare(Tr.detect("ja", ["ko-KR"]), "ja")
        compare(Tr.detect("", ["fr-FR", "zh-TW", "en"]), "zh-Hant")
        compare(Tr.detect("klingon", ["ko-KR"]), "ko")
        compare(Tr.detect("", ["fr-FR", "de"]), "en")
        compare(Tr.detect("", []), "en")
    }

    function test_the_request_comes_from_the_arguments() {
        compare(Tr.requestedIn(["prog", "--ui-language=ja", "x"]), "ja")
        compare(Tr.requestedIn(["prog", "--screenshots=/tmp"]), "")
        compare(Tr.requestedIn([]), "")
    }

    function test_every_text_has_every_translation() {
        compare(Tr.incomplete(), [])
    }

    function test_korean_is_the_source_and_others_are_looked_up() {
        Tr.use("ko")
        compare(Tr.t("일반"), "일반")
        Tr.use("ja")
        compare(Tr.t("일반"), "全般")
        Tr.use("zh-Hant")
        compare(Tr.t("일반"), "一般")
        Tr.use("zh-Hans")
        compare(Tr.t("일반"), "常规")
        Tr.use("en")
        compare(Tr.t("일반"), "General")
    }

    function test_arguments_can_be_filled_into_a_translation() {
        Tr.use("en")
        compare(Tr.t("전환 순서: %1").arg("a → b"), "Switching order: a → b")
        Tr.use("ja")
        compare(Tr.t("전환 순서: %1").arg("a → b"), "切り替え順: a → b")
    }

    function test_an_unknown_text_stays_as_it_is() {
        // Reported as MISSING-TRANSLATION on the log, which the CI looks for.
        Tr.use("en")
        compare(Tr.t("표에 없는 문구"), "표에 없는 문구")
    }

    function test_only_korean_is_not_a_draft() {
        Tr.use("ko")
        verify(!Tr.isDraft())
        Tr.use("en")
        verify(Tr.isDraft())
    }

    function cleanupTestCase() {
        Tr.use("ko")
    }
}
