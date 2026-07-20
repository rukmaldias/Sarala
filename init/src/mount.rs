//! Early filesystem mounts.
//!
//! Nothing here is optional and nothing here can be deferred: `/proc` and
//! `/sys` are how the rest of userspace observes the kernel at all, and
//! `/dev` must exist before a shell can be given a terminal.

use crate::log;
use std::ffi::CString;
use std::io;

pub struct Mount {
    pub source: &'static str,
    pub target: &'static str,
    pub fstype: &'static str,
    pub flags: libc::c_ulong,
    pub data: Option<&'static str>,
}

const NODEV_NOSUID_NOEXEC: libc::c_ulong = libc::MS_NODEV | libc::MS_NOSUID | libc::MS_NOEXEC;

/// Mounted in order. `/dev` precedes `/dev/pts` because the latter is a
/// subdirectory of the former.
pub const EARLY: &[Mount] = &[
    Mount {
        source: "proc",
        target: "/proc",
        fstype: "proc",
        flags: NODEV_NOSUID_NOEXEC,
        data: None,
    },
    Mount {
        source: "sysfs",
        target: "/sys",
        fstype: "sysfs",
        flags: NODEV_NOSUID_NOEXEC,
        data: None,
    },
    // devtmpfs, not tmpfs: the kernel populates device nodes itself, which
    // removes any need for mdev/udev this early in boot.
    Mount {
        source: "devtmpfs",
        target: "/dev",
        fstype: "devtmpfs",
        flags: libc::MS_NOSUID,
        data: Some("mode=0755"),
    },
    Mount {
        source: "devpts",
        target: "/dev/pts",
        fstype: "devpts",
        flags: libc::MS_NOSUID | libc::MS_NOEXEC,
        data: Some("mode=0620,gid=5"),
    },
    Mount {
        source: "tmpfs",
        target: "/dev/shm",
        fstype: "tmpfs",
        flags: NODEV_NOSUID_NOEXEC,
        data: Some("mode=1777"),
    },
    Mount {
        source: "tmpfs",
        target: "/run",
        fstype: "tmpfs",
        flags: NODEV_NOSUID_NOEXEC,
        data: Some("mode=0755"),
    },
];

/// Mount every spec, reporting failures rather than aborting.
///
/// A missing `/sys` is survivable and a shell on the console is worth more
/// than a clean boot — a system that dies here gives you nothing to debug
/// with. Stage 3's supervisor is where mount failure becomes fatal.
pub fn mount_all(mounts: &[Mount]) {
    for m in mounts {
        if let Err(e) = mount_one(m) {
            log::warn(&format!("mount {} on {}: {}", m.fstype, m.target, e));
        }
    }
}

fn mount_one(m: &Mount) -> io::Result<()> {
    mkdir_p(m.target)?;

    let source = CString::new(m.source)?;
    let target = CString::new(m.target)?;
    let fstype = CString::new(m.fstype)?;
    let data = m.data.map(CString::new).transpose()?;
    let data_ptr = data
        .as_ref()
        .map_or(std::ptr::null(), |d| d.as_ptr() as *const libc::c_void);

    // SAFETY: all four pointers are valid NUL-terminated C strings owned by
    // this frame and outliving the call; `data_ptr` is null or likewise valid.
    let rc = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            m.flags,
            data_ptr,
        )
    };

    if rc < 0 {
        let err = io::Error::last_os_error();
        // Already mounted is a success for our purposes — the kernel may have
        // mounted devtmpfs itself if CONFIG_DEVTMPFS_MOUNT is set.
        if err.raw_os_error() == Some(libc::EBUSY) {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

/// `mkdir -p`, treating an existing directory as success.
fn mkdir_p(path: &str) -> io::Result<()> {
    match std::fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}
