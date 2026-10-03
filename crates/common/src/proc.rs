//! Platform-specific process helpers for rendered-page scans and short-lived
//! subprocesses. Unix uses libc; Windows declares the kernel32 entry points
//! it needs directly.

use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, TryLockError};

/// `process.kill(pid, 0)`: `Ok(())` when the process exists and can be
/// signalled, otherwise the errno name Node would report (`ESRCH` when there
/// is no such process, `EPERM` when it exists but is not ours, `EINVAL`
/// otherwise). Callers that only ask "is it alive?" should use
/// [`pid_reachable`].
pub fn kill0(pid: i64) -> Result<(), &'static str> {
    if pid <= 0 || pid > i32::MAX as i64 {
        return Err("ESRCH");
    }
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if rc == 0 {
            return Ok(());
        }
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EPERM) => Err("EPERM"),
            Some(libc::ESRCH) => Err("ESRCH"),
            _ => Err("EINVAL"),
        }
    }
    #[cfg(windows)]
    {
        // libuv's uv_kill(pid, 0): OpenProcess + GetExitCodeProcess, alive
        // only while the exit code is STILL_ACTIVE. Access denied maps to
        // EPERM (the process exists), everything else to ESRCH.
        use win::*;
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32);
            if h.is_null() {
                return match GetLastError() {
                    ERROR_ACCESS_DENIED => Err("EPERM"),
                    _ => Err("ESRCH"),
                };
            }
            let mut code: u32 = 0;
            let ok = GetExitCodeProcess(h, &mut code);
            CloseHandle(h);
            if ok != 0 && code == STILL_ACTIVE {
                Ok(())
            } else {
                Err("ESRCH")
            }
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("ESRCH")
    }
}

/// `isLiveServerPidReachable(pid)` and friends: alive unless ESRCH (an EPERM
/// process is somebody else's, but it is there).
pub fn pid_reachable(pid: i64) -> bool {
    match kill0(pid) {
        Ok(()) => true,
        Err(code) => code != "ESRCH",
    }
}

/// `process.kill(pid)` (SIGTERM). On Windows Node terminates the process
/// outright; so does this. Errors are ignored, as every JS call site wraps
/// the call in `try {} catch {}`.
pub fn terminate(pid: i64) {
    if pid <= 0 || pid > i32::MAX as i64 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(windows)]
    unsafe {
        use win::*;
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid as u32);
        if !h.is_null() {
            TerminateProcess(h, 1);
            CloseHandle(h);
        }
    }
}

/// `spawn(cmd, args, { windowsHide: true })` for short-lived helpers
/// (`node --check`, `where`, `git`): on Windows a GUI-launched parent would
/// otherwise flash a console window per child. No effect elsewhere.
pub fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(win::CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Temporarily observe Ctrl-C and termination signals, restoring the prior
/// process handlers when the guard is dropped. Rendered-page scans use this
/// only for the duration of a browser session so ordinary verbs keep their
/// existing signal behavior.
pub struct InterruptGuard {
    _lock: MutexGuard<'static, ()>,
    previous_flag: *mut AtomicBool,
    #[cfg(unix)]
    previous_sigint: libc::sigaction,
    #[cfg(unix)]
    previous_sigterm: libc::sigaction,
}

impl InterruptGuard {
    pub fn install(flag: &'static AtomicBool) -> std::io::Result<Self> {
        let lock = match INTERRUPT_GUARD_LOCK.try_lock() {
            Ok(lock) => lock,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "another interrupt listener is active"));
            }
        };
        flag.store(false, std::sync::atomic::Ordering::SeqCst);
        let previous_flag = FLAG.swap(flag as *const AtomicBool as *mut AtomicBool, std::sync::atomic::Ordering::SeqCst);
        #[cfg(unix)]
        {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = unix_on_signal as *const () as libc::sighandler_t;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            action.sa_flags = libc::SA_RESTART;
            let mut previous_sigint: libc::sigaction = unsafe { std::mem::zeroed() };
            let mut previous_sigterm: libc::sigaction = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(libc::SIGINT, &action, &mut previous_sigint) } != 0 {
                let error = std::io::Error::last_os_error();
                FLAG.store(previous_flag, std::sync::atomic::Ordering::SeqCst);
                return Err(error);
            }
            if unsafe { libc::sigaction(libc::SIGTERM, &action, &mut previous_sigterm) } != 0 {
                let error = std::io::Error::last_os_error();
                unsafe {
                    libc::sigaction(libc::SIGINT, &previous_sigint, std::ptr::null_mut());
                }
                FLAG.store(previous_flag, std::sync::atomic::Ordering::SeqCst);
                return Err(error);
            }
            Ok(InterruptGuard { _lock: lock, previous_flag, previous_sigint, previous_sigterm })
        }
        #[cfg(windows)]
        {
            let ok = unsafe { win::SetConsoleCtrlHandler(Some(win_ctrl_handler), 1) };
            if ok == 0 {
                let error = std::io::Error::last_os_error();
                FLAG.store(previous_flag, std::sync::atomic::Ordering::SeqCst);
                return Err(error);
            }
            Ok(InterruptGuard { _lock: lock, previous_flag })
        }
        #[cfg(not(any(unix, windows)))]
        {
            FLAG.store(previous_flag, std::sync::atomic::Ordering::SeqCst);
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "interrupt handling is unavailable"))
        }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::sigaction(libc::SIGINT, &self.previous_sigint, std::ptr::null_mut());
            libc::sigaction(libc::SIGTERM, &self.previous_sigterm, std::ptr::null_mut());
        }
        #[cfg(windows)]
        unsafe {
            win::SetConsoleCtrlHandler(Some(win_ctrl_handler), 0);
        }
        FLAG.store(self.previous_flag, std::sync::atomic::Ordering::SeqCst);
    }
}

static FLAG: std::sync::atomic::AtomicPtr<AtomicBool> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
static INTERRUPT_GUARD_LOCK: Mutex<()> = Mutex::new(());

fn set_flag() {
    let p = FLAG.load(std::sync::atomic::Ordering::SeqCst);
    if !p.is_null() {
        // SAFETY: the pointer came from a `&'static AtomicBool`.
        unsafe { (*p).store(true, std::sync::atomic::Ordering::SeqCst) };
    }
}

#[cfg(unix)]
extern "C" fn unix_on_signal(_sig: libc::c_int) {
    set_flag();
}

#[cfg(windows)]
unsafe extern "system" fn win_ctrl_handler(_ctrl_type: u32) -> i32 {
    set_flag();
    // Handled: keep the process alive so the main loop can shut down
    // cleanly (Node's SIGINT listener has the same effect).
    1
}

#[cfg(windows)]
mod win {
    #![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]
    pub type HANDLE = *mut core::ffi::c_void;
    pub const PROCESS_TERMINATE: u32 = 0x0001;
    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    pub const STILL_ACTIVE: u32 = 259;
    pub const ERROR_ACCESS_DENIED: u32 = 5;
    pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    pub type PHANDLER_ROUTINE = Option<unsafe extern "system" fn(ctrl_type: u32) -> i32>;
    #[link(name = "kernel32")]
    extern "system" {
        pub fn OpenProcess(desired_access: u32, inherit: i32, pid: u32) -> HANDLE;
        pub fn GetExitCodeProcess(h: HANDLE, code: *mut u32) -> i32;
        pub fn TerminateProcess(h: HANDLE, exit_code: u32) -> i32;
        pub fn CloseHandle(h: HANDLE) -> i32;
        pub fn GetLastError() -> u32;
        pub fn SetConsoleCtrlHandler(handler: PHANDLER_ROUTINE, add: i32) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static INTERRUPT_TEST_FLAG: AtomicBool = AtomicBool::new(false);
    static INTERRUPT_TEST_SECOND_FLAG: AtomicBool = AtomicBool::new(false);

    #[test]
    fn own_pid_is_alive_and_bogus_pid_is_not() {
        assert_eq!(kill0(std::process::id() as i64), Ok(()));
        assert!(pid_reachable(std::process::id() as i64));
        assert_eq!(kill0(0), Err("ESRCH"));
        assert_eq!(kill0(-1), Err("ESRCH"));
        assert_eq!(kill0(i64::MAX), Err("ESRCH"));
    }

    #[test]
    fn interrupt_guard_resets_only_after_lock_and_restores_the_previous_handler() {
        INTERRUPT_TEST_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
        INTERRUPT_TEST_SECOND_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
        #[cfg(unix)]
        let previous_sigint = unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            assert_eq!(libc::sigaction(libc::SIGINT, std::ptr::null(), &mut action), 0);
            action.sa_sigaction
        };

        let guard = InterruptGuard::install(&INTERRUPT_TEST_FLAG).unwrap();
        assert!(!INTERRUPT_TEST_FLAG.load(std::sync::atomic::Ordering::SeqCst));
        #[cfg(unix)]
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            assert_eq!(libc::sigaction(libc::SIGINT, std::ptr::null(), &mut action), 0);
            assert_eq!(action.sa_sigaction, unix_on_signal as *const () as libc::sighandler_t);
        }

        INTERRUPT_TEST_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
        let nested_error = InterruptGuard::install(&INTERRUPT_TEST_SECOND_FLAG).err().expect("nested guard must be rejected");
        assert_eq!(nested_error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(INTERRUPT_TEST_FLAG.load(std::sync::atomic::Ordering::SeqCst), "a rejected nested guard cleared the active listener");
        assert!(INTERRUPT_TEST_SECOND_FLAG.load(std::sync::atomic::Ordering::SeqCst), "a rejected nested guard reset its caller's flag");
        drop(guard);

        #[cfg(unix)]
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            assert_eq!(libc::sigaction(libc::SIGINT, std::ptr::null(), &mut action), 0);
            assert_eq!(action.sa_sigaction, previous_sigint);
        }
    }
}
