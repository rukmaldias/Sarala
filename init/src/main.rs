//! Sarala PID 1.
//!
//! Stage 0 scope, and no more: mount the early filesystems, attach the
//! console, hand the operator a shell, reap orphans, and shut down cleanly.
//! There is no service supervision here — dependency ordering, restart policy
//! and log handling arrive in stage 3, once there is more than one service to
//! supervise. See `docs/roadmap.md`.

mod child;
mod console;
mod log;
mod mount;
mod signal;

use std::time::{Duration, Instant};

/// The shell handed to the operator. Busybox in stage 0; replaced when the
/// busybox dependency is demolished in stage 3.
const SHELL: &str = "/bin/sh";

/// Grace period between asking children to exit and killing them.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// A shell that dies faster than this is failing, not being exited.
const RESPAWN_FLOOR: Duration = Duration::from_secs(1);

fn main() {
    mount::mount_all(mount::EARLY);

    if let Err(e) = console::attach() {
        // Nothing can be reported if this fails — the report has nowhere to
        // go. Carry on; the kernel's own console fds may still be usable.
        let _ = e;
    }

    log::info(&format!("sarala-init {}", env!("CARGO_PKG_VERSION")));

    mark_boot_slot();

    // Must precede the first fork, so no child exit races the main loop.
    let sigset = match signal::block() {
        Ok(set) => set,
        Err(e) => {
            log::error(&format!("cannot block signals: {e}"));
            halt(libc::RB_POWER_OFF);
        }
    };

    let mut shell = Supervised::start(SHELL);

    loop {
        let sig = match signal::wait(&sigset) {
            Ok(sig) => sig,
            Err(e) => {
                log::error(&format!("sigwaitinfo: {e}"));
                continue;
            }
        };

        match sig {
            libc::SIGCHLD => {
                for r in child::reap_all() {
                    if Some(r.pid) == shell.pid {
                        log::info(&format!("shell {} {}", r.pid, r.describe()));
                        shell.restart();
                    }
                    // Any other pid is an orphan this process inherited.
                    // Reaping it was the entire obligation.
                }
            }
            libc::SIGTERM | libc::SIGINT => {
                log::info("shutting down");
                shutdown(libc::RB_POWER_OFF);
            }
            libc::SIGUSR1 => {
                log::info("rebooting");
                shutdown(libc::RB_AUTOBOOT);
            }
            other => log::warn(&format!("unexpected signal {other}")),
        }
    }
}

/// Mark the booted A/B slot good so the bootloader stops rolling back to the
/// stock slot (Sarala has no Android bootctrl HAL to do it). Best-effort: any
/// failure — no UFS, no `slot_suffix`, a transient `fastboot boot` — must never
/// stop the boot, so the result is only logged.
fn mark_boot_slot() {
    match std::process::Command::new("/sbin/bootctrl")
        .args(["mark", "/dev/sda", "auto", "--commit"])
        .status()
    {
        Ok(s) if s.success() => log::info("boot slot marked good"),
        Ok(s) => log::warn(&format!("bootctrl exited abnormally: {s}")),
        Err(e) => log::warn(&format!("bootctrl not run: {e}")),
    }
}

/// The single process stage 0 keeps alive.
struct Supervised {
    path: &'static str,
    pid: Option<libc::pid_t>,
    started: Instant,
}

impl Supervised {
    fn start(path: &'static str) -> Self {
        let mut s = Supervised {
            path,
            pid: None,
            started: Instant::now(),
        };
        s.spawn();
        s
    }

    fn spawn(&mut self) {
        self.started = Instant::now();
        match child::spawn(self.path, &[self.path, "-l"]) {
            Ok(pid) => {
                log::info(&format!("{} started as pid {}", self.path, pid));
                self.pid = Some(pid);
            }
            Err(e) => {
                log::error(&format!("cannot spawn {}: {e}", self.path));
                self.pid = None;
            }
        }
    }

    /// Restart, throttling if the shell is dying immediately.
    ///
    /// Without this a missing or broken `/bin/sh` spins PID 1 at full tilt and
    /// floods the console — the exact condition you most need to be able to
    /// read the console to diagnose.
    fn restart(&mut self) {
        if self.started.elapsed() < RESPAWN_FLOOR {
            log::warn(&format!("{} is failing fast; throttling", self.path));
            std::thread::sleep(RESPAWN_FLOOR);
        }
        self.spawn();
    }
}

/// Ask every process to exit, kill the holdouts, flush, and go down.
fn shutdown(cmd: libc::c_int) -> ! {
    // SAFETY: signalling pid -1 targets every process this one may signal.
    // PID 1 is exempt from signals it has no handler for, so this cannot
    // terminate the caller.
    unsafe {
        libc::kill(-1, libc::SIGTERM);
    }

    // Reap whatever exits during the grace period rather than sleeping
    // through it blind — most shutdowns finish well inside it.
    let deadline = Instant::now() + SHUTDOWN_GRACE;
    while Instant::now() < deadline {
        child::reap_all();
        // SAFETY: pid -1 with signal 0 probes for remaining children.
        let remaining = unsafe { libc::kill(-1, 0) };
        if remaining < 0 {
            break; // ESRCH: nothing left to wait for.
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // SAFETY: SIGKILL to the remaining processes; PID 1 remains exempt.
    unsafe {
        libc::kill(-1, libc::SIGKILL);
        libc::sync();
    }

    halt(cmd);
}

fn halt(cmd: libc::c_int) -> ! {
    // SAFETY: reboot(2) with a valid command. It does not return on success.
    unsafe {
        libc::reboot(cmd);
    }
    // Only reachable if reboot failed — spin rather than return from main,
    // since PID 1 exiting panics the kernel.
    log::error("reboot failed; halting");
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
