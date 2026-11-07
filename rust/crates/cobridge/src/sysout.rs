//! Where the COBOL programs' output goes: DISPLAY and libcob's messages, which libcob writes through the
//! C streams `stdout` and `stderr`. [`set_sysout`] points those two stream variables at a file, once per
//! process; the descriptors 1 and 2 are not touched, so Rust output (a terminal UI, a test harness, panic
//! messages) still goes where it went. A few libcob messages are written straight to descriptor 2 (the
//! `libcob: ` prefix of runtime warnings, the message of its signal handler): a full-screen UI points
//! descriptor 2 at the file while it is on ([`send_to_sysout`]).
//!
//! The stream variables can be assigned with glibc, i.e. on Linux, where the COBOL programs run (NFR-09:
//! the system builds on Linux and Windows). Elsewhere [`set_sysout`] reports that it is not supported.

pub use imp::*;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::ffi::{c_int, CString};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard, TryLockError};

    // glibc's standard stream variables (assignable, unlike a `FILE *const`)
    extern "C" {
        #[link_name = "stdout"]
        static mut C_STDOUT: *mut libc::FILE;
        #[link_name = "stderr"]
        static mut C_STDERR: *mut libc::FILE;
    }

    /// The sysout file: one C stream for the whole process (reopened when the file changes, never closed:
    /// libcob keeps the stream it was given), and its path for messages that point the user to it.
    struct Sysout {
        file: *mut libc::FILE,
        path: PathBuf,
    }

    // The stream is only used under the lock (and by libcob, which runs one program at a time).
    unsafe impl Send for Sysout {}

    static SYSOUT: Mutex<Option<Sysout>> = Mutex::new(None);

    /// The sysout state. Never held across a COBOL call; a poisoned lock still holds a valid stream.
    fn sysout() -> MutexGuard<'static, Option<Sysout>> {
        SYSOUT.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Send the COBOL programs' output to `path` (appended), for the rest of the process (see the module
    /// documentation). Calling it again switches the same stream to another file.
    pub fn set_sysout(path: &Path) -> std::io::Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let name = CString::new(path.as_os_str().as_bytes()).map_err(std::io::Error::other)?;
        let mut st = sysout();
        if st.as_ref().is_some_and(|s| s.path == path) {
            return Ok(()); // already there
        }
        let file = match st.as_ref() {
            Some(s) => unsafe {
                libc::fflush(s.file);
                libc::freopen(name.as_ptr(), c"a".as_ptr(), s.file)
            },
            None => unsafe { libc::fopen(name.as_ptr(), c"a".as_ptr()) },
        };
        if file.is_null() {
            // a failed freopen has closed the stream: forget it (the C stream variables still point at it,
            // so libcob's output is lost until the next successful call)
            let err = std::io::Error::last_os_error();
            *st = None;
            return Err(err);
        }
        unsafe {
            C_STDOUT = file;
            C_STDERR = file;
        }
        *st = Some(Sysout { file, path: path.to_path_buf() });
        Ok(())
    }

    /// The file given to [`set_sysout`], if any.
    pub fn sysout_path() -> Option<PathBuf> {
        sysout().as_ref().map(|s| s.path.clone())
    }

    /// Append `text` to the sysout file (flushed). False if there is none, or if another thread holds it
    /// (never blocks: a panic hook calls this).
    pub fn write_sysout(text: &str) -> bool {
        let st = match SYSOUT.try_lock() {
            Ok(g) => g,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
        };
        let Some(s) = st.as_ref() else { return false };
        unsafe {
            libc::fwrite(text.as_ptr().cast(), 1, text.len(), s.file);
            libc::fflush(s.file);
        }
        true
    }

    /// Point descriptor `fd` at the sysout file (for a UI's descriptor 2 while it is on). Ok(false) if
    /// there is no sysout file.
    pub fn send_to_sysout(fd: c_int) -> std::io::Result<bool> {
        let st = sysout();
        let Some(s) = st.as_ref() else { return Ok(false) };
        if unsafe { libc::dup2(libc::fileno(s.file), fd) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(true)
    }

    /// Write out what the COBOL programs have buffered.
    pub(crate) fn flush() {
        if let Some(s) = sysout().as_ref() {
            unsafe { libc::fflush(s.file) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Device and inode of fd 1: which file it really points at.
        fn stdout_id() -> (u64, u64) {
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            assert_eq!(unsafe { libc::fstat(1, &mut st) }, 0);
            (st.st_dev as u64, st.st_ino as u64)
        }

        #[test]
        fn sysout_takes_c_stdio_and_leaves_descriptors_alone() {
            let dir = std::env::temp_dir();
            let (a, b) = (dir.join(format!("cobridge-a-{}.log", std::process::id())), dir.join(format!("cobridge-b-{}.log", std::process::id())));
            for p in [&a, &b] {
                let _ = std::fs::remove_file(p);
            }
            let (orig_out, orig_err) = unsafe { (C_STDOUT, C_STDERR) };
            let before = stdout_id();
            set_sysout(&a).unwrap();
            unsafe {
                libc::fputs(c"to stdout\n".as_ptr(), C_STDOUT);
                libc::fputs(c"to stderr\n".as_ptr(), C_STDERR);
            }
            flush();
            assert_eq!(stdout_id(), before, "fd 1 untouched");
            assert!(write_sysout("from a hook\n"));
            assert_eq!(std::fs::read_to_string(&a).unwrap(), "to stdout\nto stderr\nfrom a hook\n");
            assert_eq!(sysout_path().as_deref(), Some(a.as_path()));
            set_sysout(&a).unwrap(); // the same file again: nothing changes
            set_sysout(&b).unwrap(); // switching files
            unsafe { libc::fputs(c"later\n".as_ptr(), C_STDOUT) };
            flush();
            assert_eq!(std::fs::read_to_string(&b).unwrap(), "later\n");
            assert_eq!(sysout_path().as_deref(), Some(b.as_path()));
            // leave the process as it was: the original C streams, no sysout file
            unsafe {
                C_STDOUT = orig_out;
                C_STDERR = orig_err;
                if let Some(s) = sysout().take() {
                    libc::fclose(s.file); // no libcob in this test binary to hold on to it
                }
            }
            for p in [a, b] {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod imp {
    use std::ffi::c_int;
    use std::path::{Path, PathBuf};

    /// Not supported here: the C stream variables cannot be assigned (see the module documentation).
    pub fn set_sysout(_path: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "the COBOL programs run on Linux (glibc) only",
        ))
    }

    pub fn sysout_path() -> Option<PathBuf> {
        None
    }

    pub fn write_sysout(_text: &str) -> bool {
        false
    }

    pub fn send_to_sysout(_fd: c_int) -> std::io::Result<bool> {
        Ok(false)
    }

    pub(crate) fn flush() {}
}
