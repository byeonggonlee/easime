//! Asking the running input method to do something from another thread, the
//! tray icon for instance ("switch to Japanese").
//!
//! The input method is one thread waiting in `poll` for Wayland and its
//! timers. A request is put in a queue and an `eventfd` is written to, which
//! wakes that poll; the input thread then takes the requests and carries them
//! out itself, so nothing else ever touches the session.
//!
//! **OS-specific: Linux** (`eventfd`, polled with the Wayland socket). Other
//! OSes wake their own message loop (`docs/platforms.md`).

use ime_core::Language;
use rustix::event::{EventfdFlags, eventfd};
use std::collections::VecDeque;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::{Mutex, OnceLock};

/// Something asked of the input method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Switch to this language.
    Select(Language),
    /// Switch to the next language, as the switch key does.
    Next,
}

struct Channel {
    queue: Mutex<VecDeque<Command>>,
    wake: OwnedFd,
}

/// One per process; `None` if the system gave no eventfd (requests are then
/// dropped, and the input method itself does not need them).
fn channel() -> Option<&'static Channel> {
    static CHANNEL: OnceLock<Option<Channel>> = OnceLock::new();
    CHANNEL
        .get_or_init(|| {
            let wake = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).ok()?;
            Some(Channel {
                queue: Mutex::new(VecDeque::new()),
                wake,
            })
        })
        .as_ref()
}

/// Ask the input method to do `command`. Never blocks.
pub fn send(command: Command) {
    let Some(channel) = channel() else {
        return;
    };
    if let Ok(mut queue) = channel.queue.lock() {
        queue.push_back(command);
    }
    // Wake the input thread. A full counter (it has not run for 2^64 requests)
    // or an error cannot lose the request: it is in the queue.
    let _ = rustix::io::write(&channel.wake, &1u64.to_ne_bytes());
}

/// What the input thread polls for.
pub(crate) fn wake_fd() -> Option<BorrowedFd<'static>> {
    channel().map(|channel| channel.wake.as_fd())
}

/// Take everything asked since last time and clear the wake-up.
pub(crate) fn take() -> Vec<Command> {
    let Some(channel) = channel() else {
        return Vec::new();
    };
    let mut counter = [0u8; 8];
    let _ = rustix::io::read(&channel.wake, &mut counter);
    channel
        .queue
        .lock()
        .map(|mut queue| queue.drain(..).collect())
        .unwrap_or_default()
}
