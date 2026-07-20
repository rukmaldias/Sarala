//! Forking and reaping children.

use crate::signal;
use std::ffi::CString;
use std::io;

/// Spawn a program as a session leader with the console as its controlling
/// terminal, so job control and Ctrl-C work in the resulting shell.
///
/// Every allocation happens before the fork: between fork and exec only
/// async-signal-safe calls are legal, and allocating there can deadlock on a
/// malloc lock held by another thread at fork time.
pub fn spawn(path: &str, args: &[&str]) -> io::Result<libc::pid_t> {
    let c_path = CString::new(path)?;
    let c_args: Vec<CString> = args
        .iter()
        .map(|a| CString::new(*a))
        .collect::<Result<_, _>>()?;
    let mut argv: Vec<*const libc::c_char> = c_args.iter().map(|a| a.as_ptr()).collect();
    argv.push(std::ptr::null());

    // A shell needs at least this much to behave like a login shell.
    let c_env: Vec<CString> = ["HOME=/root", "PATH=/bin:/sbin", "TERM=linux"]
        .iter()
        .map(|e| CString::new(*e))
        .collect::<Result<_, _>>()?;
    let mut envp: Vec<*const libc::c_char> = c_env.iter().map(|e| e.as_ptr()).collect();
    envp.push(std::ptr::null());

    // SAFETY: this process is single-threaded, so the child may safely run
    // until execve. The child path below performs no allocation.
    let pid = unsafe { libc::fork() };
    match pid {
        -1 => Err(io::Error::last_os_error()),
        0 => {
            // Child. Nothing here may return — every path ends in _exit.
            let _ = signal::unblock_all();

            // SAFETY: async-signal-safe syscalls only; all pointers were
            // built before the fork and remain valid in this address space.
            unsafe {
                // New session, then claim the console as controlling terminal.
                libc::setsid();
                libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY, 0);
                libc::execve(c_path.as_ptr(), argv.as_ptr(), envp.as_ptr());
                // execve only returns on failure.
                libc::_exit(127);
            }
        }
        pid => Ok(pid),
    }
}

/// Outcome of a single reap.
pub struct Reaped {
    pub pid: libc::pid_t,
    pub status: libc::c_int,
}

impl Reaped {
    /// Human-readable exit description for the log.
    pub fn describe(&self) -> String {
        if libc::WIFEXITED(self.status) {
            format!("exited with code {}", libc::WEXITSTATUS(self.status))
        } else if libc::WIFSIGNALED(self.status) {
            format!("killed by signal {}", libc::WTERMSIG(self.status))
        } else {
            format!("terminated with raw status {}", self.status)
        }
    }
}

/// Reap every child that has exited, without blocking.
///
/// PID 1 inherits every orphan on the system, so this drains rather than
/// reaping once: a single `SIGCHLD` can stand for several exits, because
/// signals do not queue while one of the same number is already pending.
pub fn reap_all() -> Vec<Reaped> {
    let mut reaped = Vec::new();
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `status` is a valid writable int; -1 waits for any child.
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid <= 0 {
            // 0: children exist but none have exited. -1: no children at all.
            return reaped;
        }
        reaped.push(Reaped { pid, status });
    }
}
