//! The easime daemon. KWin starts it as the configured virtual keyboard.

use frontend_wayland::RunError;

fn main() {
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
