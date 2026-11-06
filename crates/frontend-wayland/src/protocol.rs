//! Generated bindings for `input-method-unstable-v1`, the protocol KWin uses
//! to talk to an input method process. (Not part of `wayland-protocols`.)

#![allow(
    clippy::all,
    non_upper_case_globals,
    non_camel_case_types,
    missing_docs
)]

pub mod client {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/input-method-unstable-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/input-method-unstable-v1.xml");
}
