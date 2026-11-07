//! The easime daemon. KWin starts it as the configured virtual keyboard.

use frontend_wayland::RunError;

fn main() {
    // The task bar icon follows the active language. The input method only
    // sends it a message; the icon is drawn and served on its own thread.
    let tray = easime::tray::start();
    frontend_wayland::observe_language(move |language, cycle| {
        let _ = tray.send((language, cycle.to_vec()));
    });
    let Err(error) = frontend_wayland::run();
    frontend_wayland::log!("fatal: {error}");
    eprintln!("easime: {error}");
    match error {
        // Restarting cannot fix this, so exit normally and let KWin leave us.
        RunError::Startup(_) => std::process::exit(1),
        // KWin only restarts an input method that terminated abnormally, so
        // abort (as a panic does, see `panic = "abort"` in Cargo.toml).
        RunError::Runtime(_) => std::process::abort(),
    }
}
