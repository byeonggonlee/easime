//! The operating systems easime runs on, for the few places where a rule
//! differs per OS (where files live, how the system language is read).
//!
//! Such code takes an [`Os`] as an argument instead of testing the build target
//! itself, so every OS's rule is unit-tested on every OS. See
//! `docs/platforms.md`.

/// An operating system with its own conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

impl Os {
    /// The OS this program was built for. Anything that is not Windows or macOS
    /// follows the Linux (XDG) conventions.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_os_matches_the_build_target() {
        let expected = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        };
        assert_eq!(Os::current(), expected);
    }
}
