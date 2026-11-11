//! Puts the commit the DLL was built from into it (`EASIME_COMMIT`), so that the
//! log says which build a program is running. The installer never closes the
//! programs that use easime, so after an upgrade a program may still be running
//! the old DLL until it is started again; the log line of its start tells.

use std::process::Command;

fn main() {
    let from_ci = std::env::var("GITHUB_SHA")
        .ok()
        .map(|sha| sha.chars().take(7).collect::<String>())
        .filter(|sha| !sha.is_empty());
    let commit = from_ci
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=7", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                .filter(|sha| !sha.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=EASIME_COMMIT={commit}");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
