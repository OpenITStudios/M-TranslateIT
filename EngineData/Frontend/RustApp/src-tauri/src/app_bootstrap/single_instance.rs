//! Windows application-level admission. This guard is acquired before Tauri
//! creates windows, startup markers, shared data paths, workers or audio owners.
//! It is not a Meeting/session lock and must not start a second runtime.

use std::collections::HashSet;
use std::ffi::c_void;
use std::io;
use std::ptr;

use sysinfo::System;

type Handle = *mut c_void;
const MUTEX_NAME: &str = "Local\\HaloKaryaMedia.TranslateIT.SingleInstance.v1";
const MAIN_TITLE: &str = "TranslateIT";
const ERROR_ALREADY_EXISTS: u32 = 183;
const GW_OWNER: u32 = 4;
const SW_RESTORE: i32 = 9;
const MB_ICONERROR: u32 = 0x0000_0010;
const MB_ICONINFORMATION: u32 = 0x0000_0040;

#[link(name = "Kernel32")]
extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: Handle) -> i32;
}

#[link(name = "User32")]
extern "system" {
    fn EnumWindows(callback: Option<unsafe extern "system" fn(Handle, isize) -> i32>, data: isize) -> i32;
    fn GetWindowThreadProcessId(window: Handle, process_id: *mut u32) -> u32;
    fn GetWindow(window: Handle, command: u32) -> Handle;
    fn GetWindowTextW(window: Handle, text: *mut u16, capacity: i32) -> i32;
    fn ShowWindow(window: Handle, command: i32) -> i32;
    fn SetForegroundWindow(window: Handle) -> i32;
    fn FlashWindow(window: Handle, invert: i32) -> i32;
    fn MessageBoxW(window: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

pub(crate) struct PrimaryInstance {
    mutex: Handle,
}

impl Drop for PrimaryInstance {
    fn drop(&mut self) {
        // The kernel also closes non-inherited handles on a hard process exit.
        unsafe { CloseHandle(self.mutex) };
    }
}

pub(crate) enum InstanceAdmission {
    Primary(PrimaryInstance),
    Existing,
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn show_notice(message: &str, kind: u32) {
    let body = wide_null(message);
    let title = wide_null(MAIN_TITLE);
    unsafe { MessageBoxW(ptr::null_mut(), body.as_ptr(), title.as_ptr(), kind) };
}

struct WindowSearch<'a> {
    eligible_pids: &'a HashSet<u32>,
    found: Handle,
}

unsafe extern "system" fn find_matching_main(window: Handle, opaque: isize) -> i32 {
    // EnumWindows invokes this callback synchronously while its stack owner lives.
    let search = unsafe { &mut *(opaque as *mut WindowSearch<'_>) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if !search.eligible_pids.contains(&pid)
        || !unsafe { GetWindow(window, GW_OWNER) }.is_null()
    {
        return 1;
    }

    let mut title = [0u16; 64];
    let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
    if length <= 0 || String::from_utf16_lossy(&title[..length as usize]) != MAIN_TITLE {
        return 1;
    }
    search.found = window;
    0 // Stop enumeration at the matching top-level main window.
}

fn restore_existing_main() -> bool {
    // Do not raise a window on its title alone: require an exact executable
    // identity. Only a collided second launch takes this (non-hot) path.
    let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok())
    else {
        return false;
    };
    let own_pid = std::process::id();
    let system = System::new_all();
    let eligible_pids: HashSet<u32> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            if pid.as_u32() == own_pid {
                return None;
            }
            let candidate = std::fs::canonicalize(process.exe()?).ok()?;
            (candidate == exe).then_some(pid.as_u32())
        })
        .collect();
    if eligible_pids.is_empty() {
        return false;
    }

    let mut search = WindowSearch {
        eligible_pids: &eligible_pids,
        found: ptr::null_mut(),
    };
    unsafe {
        EnumWindows(
            Some(find_matching_main),
            (&mut search as *mut WindowSearch<'_>) as isize,
        )
    };
    if search.found.is_null() {
        return false; // Primary may still be constructing its first window.
    }
    unsafe {
        ShowWindow(search.found, SW_RESTORE);
        if SetForegroundWindow(search.found) == 0 {
            // Windows may deny foreground activation; signal the taskbar instead.
            FlashWindow(search.found, 1);
        }
    }
    true
}

pub(crate) fn acquire() -> io::Result<InstanceAdmission> {
    // Local\ scopes the object to the interactive Windows session. No file
    // marker or timeout can outlive the owning process and block crash recovery.
    let name = wide_null(MUTEX_NAME);
    let mutex = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() {
        return Err(io::Error::last_os_error()); // Fail closed; never start unguarded.
    }
    // Read immediately after CreateMutexW, before another OS call overwrites it.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if existed {
        unsafe { CloseHandle(mutex) };
        if !restore_existing_main() {
            show_notice(
                "TranslateIT is already running or starting. Use its existing window.",
                MB_ICONINFORMATION,
            );
        }
        return Ok(InstanceAdmission::Existing);
    }
    Ok(InstanceAdmission::Primary(PrimaryInstance { mutex }))
}

pub(crate) fn show_startup_error(error: &io::Error) {
    eprintln!("TranslateIT single-instance admission failed: {error}");
    show_notice(
        "TranslateIT could not safely verify that no other instance is running. A second instance was not started.",
        MB_ICONERROR,
    );
}
