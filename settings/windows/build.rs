//! The Windows App Runtime goes next to the program (the installer ships it:
//! docs/decisions.md, "설정 GUI"). `windows-reactor-setup` downloads it from
//! NuGet, copies its files into the target folder and embeds the manifest that
//! tells the program to use those files. Only when building on Windows for
//! Windows: elsewhere the app is only type checked.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_reactor_setup::as_self_contained();
    }
}
