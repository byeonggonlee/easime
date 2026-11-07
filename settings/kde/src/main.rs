//! The easime settings window (expert version: every option on one of a few
//! pages, saved as soon as it changes).

mod backend;
mod model;
mod view;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    // Look like the rest of Plasma unless the user chose a style.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/easime/settings/qml/Main.qml"));
    }
    let code = app.as_mut().map_or(1, |app| app.exec());
    std::process::exit(code);
}
