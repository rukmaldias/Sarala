//! Console setup.
//!
//! The kernel hands PID 1 whatever `console=` pointed at, but only if it was
//! opened — an initramfs `/init` starts with fds 0/1/2 already wired to the
//! console in most configurations and with nothing at all in some. Opening
//! `/dev/console` explicitly makes the outcome the same either way.

use std::ffi::CString;
use std::io;

/// Point stdin, stdout and stderr at `/dev/console`.
///
/// Requires `/dev` to be mounted first.
pub fn attach() -> io::Result<()> {
    let path = CString::new("/dev/console").expect("static string has no NUL");

    // SAFETY: `path` is a valid NUL-terminated string living past the call.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    for target in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        // SAFETY: `fd` is open and owned here; `target` is a valid fd number.
        if unsafe { libc::dup2(fd, target) } < 0 {
            let err = io::Error::last_os_error();
            // SAFETY: `fd` is open and not otherwise closed on this path.
            unsafe { libc::close(fd) };
            return Err(err);
        }
    }

    if fd > libc::STDERR_FILENO {
        // SAFETY: `fd` is open, owned, and now redundant after the dup2s.
        unsafe { libc::close(fd) };
    }
    Ok(())
}
