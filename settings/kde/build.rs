use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.easime.settings").qml_files([
        "qml/Main.qml",
        "qml/Messages.qml",
        "qml/Choice.qml",
        "qml/GeneralPage.qml",
        "qml/LanguagesPage.qml",
        "qml/ShortcutsPage.qml",
        "qml/KoreanPage.qml",
        "qml/AppearancePage.qml",
    ]))
    .qt_module("Quick")
    .file("src/backend.rs")
    .build();
}
