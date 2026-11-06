//! A repeating timer backed by `timerfd`, so it can be polled together with
//! the Wayland connection.

use rustix::io::Errno;
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::Duration;

pub struct Timer {
    fd: OwnedFd,
}

fn timespec(d: Duration) -> Timespec {
    Timespec {
        tv_sec: d.as_secs() as _,
        tv_nsec: d.subsec_nanos() as _,
    }
}

impl Timer {
    pub fn new() -> io::Result<Self> {
        let fd = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        Ok(Self { fd })
    }

    /// Fire once after `delay`, then every `interval`.
    pub fn arm(&self, delay: Duration, interval: Duration) -> io::Result<()> {
        // An all-zero `it_value` would disarm the timer, so use at least 1 ns.
        let delay = delay.max(Duration::from_nanos(1));
        let spec = Itimerspec {
            it_interval: timespec(interval),
            it_value: timespec(delay),
        };
        timerfd_settime(&self.fd, TimerfdTimerFlags::empty(), &spec)?;
        Ok(())
    }

    pub fn disarm(&self) -> io::Result<()> {
        let spec = Itimerspec {
            it_interval: timespec(Duration::ZERO),
            it_value: timespec(Duration::ZERO),
        };
        timerfd_settime(&self.fd, TimerfdTimerFlags::empty(), &spec)?;
        Ok(())
    }

    /// Read the expiration count. Returns 0 if the timer has not fired.
    pub fn take_expirations(&self) -> io::Result<u64> {
        let mut buf = [0u8; 8];
        match rustix::io::read(&self.fd, &mut buf) {
            Ok(8) => Ok(u64::from_ne_bytes(buf)),
            Ok(_) => Ok(0),
            Err(Errno::AGAIN) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::event::{PollFd, PollFlags, poll};

    /// Wait until `timer` is readable or `timeout` passes.
    fn wait_readable(timer: &Timer, timeout: Duration) -> bool {
        let mut fds = [PollFd::new(&timer.fd, PollFlags::IN)];
        let ts = timespec(timeout);
        matches!(poll(&mut fds, Some(&ts)), Ok(n) if n > 0)
    }

    #[test]
    fn unarmed_timer_never_fires() {
        let t = Timer::new().unwrap();
        assert!(!wait_readable(&t, Duration::from_millis(30)));
        assert_eq!(t.take_expirations().unwrap(), 0);
    }

    #[test]
    fn fires_after_delay() {
        let t = Timer::new().unwrap();
        t.arm(Duration::from_millis(5), Duration::from_millis(5))
            .unwrap();
        assert!(wait_readable(&t, Duration::from_secs(2)));
        assert!(t.take_expirations().unwrap() >= 1);
    }

    #[test]
    fn disarm_stops_firing() {
        let t = Timer::new().unwrap();
        t.arm(Duration::from_millis(50), Duration::from_millis(50))
            .unwrap();
        t.disarm().unwrap();
        assert!(!wait_readable(&t, Duration::from_millis(120)));
    }
}
