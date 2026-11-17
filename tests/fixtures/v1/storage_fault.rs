//! Linux/glibc test-only interposer. Compile with rustc --crate-type cdylib.
//! Only a subprocess with QUIET_FAULT_ROOT set can be affected. No product hooks.
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    sync::atomic::{AtomicU8, Ordering},
};
#[link(name = "dl")]
extern "C" {
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}
extern "C" {
    fn __errno_location() -> *mut c_int;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn getpid() -> c_int;
}
static PHASE: AtomicU8 = AtomicU8::new(0); // backup, backup-replaced, primary, primary-replaced
static FIRED: AtomicU8 = AtomicU8::new(0);
unsafe fn symbol(name: &[u8]) -> *mut c_void {
    let p = dlsym((-1isize) as *mut c_void, name.as_ptr().cast());
    if p.is_null() {
        std::process::abort();
    }
    p
}
fn private(path: &std::path::Path) -> bool {
    std::env::var_os("QUIET_FAULT_ROOT").is_some_and(|root| path.starts_with(root))
}
fn descriptor(fd: c_int) -> Option<std::path::PathBuf> {
    let p = std::fs::read_link(format!("/proc/self/fd/{fd}")).ok()?;
    private(&p).then_some(p)
}
unsafe fn trigger(stage: &str) -> bool {
    if std::env::var("QUIET_FAULT_STAGE").ok().as_deref() != Some(stage)
        || FIRED.swap(1, Ordering::SeqCst) != 0
    {
        return false;
    }
    let real: unsafe extern "C" fn(c_int, *const c_void, usize) -> isize =
        std::mem::transmute(symbol(b"write\0"));
    let marker = format!("FAULT_FIRED:{stage}\n");
    real(2, marker.as_ptr().cast(), marker.len());
    if std::env::var("QUIET_FAULT_ACTION").ok().as_deref() == Some("kill") {
        kill(getpid(), 9);
        std::process::abort();
    }
    *__errno_location() = 5; // EIO
    true
}
#[no_mangle]
pub unsafe extern "C" fn write(fd: c_int, p: *const c_void, len: usize) -> isize {
    let real: unsafe extern "C" fn(c_int, *const c_void, usize) -> isize =
        std::mem::transmute(symbol(b"write\0"));
    if descriptor(fd).is_some_and(|p| p.extension().is_some_and(|e| e == "tmp")) {
        let stage = if PHASE.load(Ordering::SeqCst) < 2 {
            "backup.write"
        } else {
            "primary.write"
        };
        if trigger(stage) {
            return -1;
        }
    }
    real(fd, p, len)
}
#[no_mangle]
pub unsafe extern "C" fn fsync(fd: c_int) -> c_int {
    let real: unsafe extern "C" fn(c_int) -> c_int = std::mem::transmute(symbol(b"fsync\0"));
    if let Some(p) = descriptor(fd) {
        let phase = PHASE.load(Ordering::SeqCst);
        let stage = if p.is_dir() {
            match phase {
                0 => "backup.directory_before_replace",
                1 => "backup.directory_after_replace",
                2 => "primary.directory_before_replace",
                _ => "primary.directory_after_replace",
            }
        } else if phase < 2 {
            "backup.file_sync"
        } else {
            "primary.file_sync"
        };
        if trigger(stage) {
            return -1;
        }
        let result = real(fd);
        if result == 0 && p.is_dir() && phase == 1 {
            PHASE.store(2, Ordering::SeqCst);
        }
        return result;
    }
    real(fd)
}
#[no_mangle]
pub unsafe extern "C" fn rename(from: *const c_char, to: *const c_char) -> c_int {
    let real: unsafe extern "C" fn(*const c_char, *const c_char) -> c_int =
        std::mem::transmute(symbol(b"rename\0"));
    let to_path = std::path::Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
        CStr::from_ptr(to).to_bytes(),
    ));
    if private(to_path) {
        let backup = to_path
            .file_name()
            .is_some_and(|n| n == "settings.json.bak");
        let primary = to_path.file_name().is_some_and(|n| n == "settings.json");
        if backup || primary {
            if trigger(if backup {
                "backup.replace"
            } else {
                "primary.replace"
            }) {
                return -1;
            }
            let result = real(from, to);
            if result == 0 {
                PHASE.store(if backup { 1 } else { 3 }, Ordering::SeqCst);
            }
            return result;
        }
    }
    real(from, to)
}
