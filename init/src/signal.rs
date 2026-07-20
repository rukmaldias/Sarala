//! Signal handling for PID 1.
//!
//! PID 1 is special: the kernel delivers no default action for signals it has
//! not explicitly handled, so the usual "install a handler" reflex is both
//! unnecessary and racy. Instead the interesting signals are blocked process
//! wide and consumed synchronously with `sigwaitinfo`, which turns
//! asynchronous delivery into an ordinary blocking read in the main loop.

use std::io;
use std::mem::MaybeUninit;

/// Signals PID 1 consumes synchronously.
pub const HANDLED: &[libc::c_int] = &[libc::SIGCHLD, libc::SIGTERM, libc::SIGINT, libc::SIGUSR1];

/// Build a `sigset_t` containing [`HANDLED`].
fn handled_set() -> libc::sigset_t {
    let mut set = MaybeUninit::<libc::sigset_t>::uninit();
    // SAFETY: sigemptyset initialises the set it is given; every subsequent
    // sigaddset writes to a now-initialised set with a valid signal number.
    unsafe {
        libc::sigemptyset(set.as_mut_ptr());
        for &sig in HANDLED {
            libc::sigaddset(set.as_mut_ptr(), sig);
        }
        set.assume_init()
    }
}

/// Block [`HANDLED`] for this process.
///
/// Must be called before forking any child, so no signal can be delivered
/// between the fork and the main loop's first `wait`.
pub fn block() -> io::Result<libc::sigset_t> {
    let set = handled_set();
    // SAFETY: `set` is initialised; the null oldset pointer is permitted.
    let rc = unsafe { libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(set)
}

/// Restore the default (empty) signal mask.
///
/// The mask survives `execve`, so every child must call this between fork and
/// exec — otherwise the spawned shell inherits a blocked `SIGCHLD` and its own
/// job control silently breaks.
pub fn unblock_all() -> io::Result<()> {
    let mut set = MaybeUninit::<libc::sigset_t>::uninit();
    // SAFETY: sigemptyset initialises the set before sigprocmask reads it.
    let rc = unsafe {
        libc::sigemptyset(set.as_mut_ptr());
        libc::sigprocmask(libc::SIG_SETMASK, set.as_ptr(), std::ptr::null_mut())
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Block until one of [`HANDLED`] arrives, returning its signal number.
///
/// Retries on `EINTR`, which an unblocked signal outside the set can cause.
pub fn wait(set: &libc::sigset_t) -> io::Result<libc::c_int> {
    loop {
        let mut info = MaybeUninit::<libc::siginfo_t>::uninit();
        // SAFETY: `set` is a valid initialised sigset; `info` is a valid
        // writable siginfo_t that the kernel fills on success.
        let rc = unsafe { libc::sigwaitinfo(set, info.as_mut_ptr()) };
        if rc >= 0 {
            return Ok(rc);
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(err);
    }
}
