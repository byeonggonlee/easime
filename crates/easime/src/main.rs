//! The easime daemon. KWin starts it as the configured virtual keyboard.

fn main() {
    if let Err(e) = frontend_wayland::run() {
        frontend_wayland::log!("fatal: {e}");
        eprintln!("easime: {e}");
        std::process::exit(1);
    }
}
