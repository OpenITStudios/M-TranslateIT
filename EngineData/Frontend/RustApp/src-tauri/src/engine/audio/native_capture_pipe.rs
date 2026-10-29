//! Candidate unprivileged client for the *authenticated* TranslateIT broker.
//! This module never connects from normal Meeting until independent Windows
//! security, WDK, pacing and receiver acceptance exist.
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind};
use std::time::{Duration, Instant};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::ptr;

use super::native_capture_transport::{
    begin_bytes, frame_bytes, parse_status, revoke_bytes, NativeCaptureStatus,
};

const PIPE: &str = r"\\.\pipe\TranslateITMicBrokerV1";
const SERVICE_NAME: &str = "TranslateITMicBroker";
// The client requests only FILE_GENERIC_READ + FILE_WRITE_DATA.
// FILE_CREATE_PIPE_INSTANCE (0x4) must never be granted to users.
const CLIENT_ACCESS: u32 = 0x0012_008B;
const MAGIC: u32 = 0x5242_4954; // little-endian TIBR
const VERSION: u16 = 1;
const MAX_PAYLOAD: usize = 1952;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const ERROR_IO_PENDING: i32 = 997;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const IPC_TIMEOUT: Duration = Duration::from_millis(2_000);
const BEGIN: u16 = 1;
const WRITE: u16 = 2;
const FLUSH: u16 = 3;
const END: u16 = 4;
const STATUS: u16 = 5;
const SC_MANAGER_CONNECT: u32 = 0x0001;
const SERVICE_QUERY_STATUS: u32 = 0x0004;
const SERVICE_RUNNING: u32 = 0x0004;
const SERVICE_WIN32_OWN_PROCESS: u32 = 0x0010;

#[repr(C)]
#[derive(Default)]
struct ServiceStatusProcess {
    service_type: u32,
    current_state: u32,
    controls_accepted: u32,
    win32_exit_code: u32,
    service_specific_exit_code: u32,
    check_point: u32,
    wait_hint: u32,
    process_id: u32,
    service_flags: u32,
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> *mut c_void;
    fn OpenServiceW(manager: *mut c_void, service: *const u16, access: u32) -> *mut c_void;
    fn QueryServiceStatusEx(service: *mut c_void, info_level: u32, buffer: *mut u8, size: u32, needed: *mut u32) -> i32;
    fn CloseServiceHandle(handle: *mut c_void) -> i32;
}
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: *mut c_void,
}
impl Default for Overlapped {
    fn default() -> Self {
        Self {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: ptr::null_mut(),
        }
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetNamedPipeServerProcessId(pipe: *mut c_void, process_id: *mut u32) -> i32;
    fn CreateEventW(attributes: *mut c_void, manual_reset: i32, initial_state: i32, name: *const u16) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn ReadFile(handle: *mut c_void, buffer: *mut c_void, bytes: u32, transferred: *mut u32, op: *mut Overlapped) -> i32;
    fn WriteFile(handle: *mut c_void, buffer: *const c_void, bytes: u32, transferred: *mut u32, op: *mut Overlapped) -> i32;
    fn GetOverlappedResult(handle: *mut c_void, op: *mut Overlapped, transferred: *mut u32, wait: i32) -> i32;
    fn CancelIoEx(handle: *mut c_void, op: *mut Overlapped) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
}

struct ScHandle(*mut c_void);
impl Drop for ScHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: owns one SCM handle returned by OpenSCManager/OpenService.
            unsafe { CloseServiceHandle(self.0); }
        }
    }
}

fn broker_error(message: &'static str) -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, message)
}

/// Must run BEFORE any microphone PCM is written to the pipe. A malicious
/// same-named user pipe cannot impersonate a registered running SCM service.
fn verify_service_process(file: &File) -> io::Result<()> {
    // SAFETY: all pointers live for this synchronous Win32 call; SCM handles
    // are closed on every exit. No elevation is required for QUERY_STATUS.
    unsafe {
        let scm = ScHandle(OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT));
        if scm.0.is_null() {
            return Err(broker_error("native_capture:scm_unavailable"));
        }
        let wide: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let service = ScHandle(OpenServiceW(scm.0, wide.as_ptr(), SERVICE_QUERY_STATUS));
        if service.0.is_null() {
            return Err(broker_error("native_capture:broker_service_missing"));
        }
        let mut info = ServiceStatusProcess::default();
        let mut required = 0;
        if QueryServiceStatusEx(service.0, 0,
            (&mut info as *mut ServiceStatusProcess).cast::<u8>(),
            u32::try_from(std::mem::size_of::<ServiceStatusProcess>()).unwrap(),
            &mut required) == 0
            || info.current_state != SERVICE_RUNNING
            || info.service_type != SERVICE_WIN32_OWN_PROCESS
            || info.process_id == 0
        {
            return Err(broker_error("native_capture:broker_service_not_running"));
        }
        let mut actual_pid = 0;
        if GetNamedPipeServerProcessId(file.as_raw_handle().cast(), &mut actual_pid) == 0
            || actual_pid != info.process_id
        {
            return Err(broker_error("native_capture:broker_pipe_server_not_trusted"));
        }
    }
    Ok(())
}

fn request_bytes(operation: u16, payload: &[u8]) -> io::Result<[u8; 16]> {
    if !(BEGIN..=STATUS).contains(&operation) || payload.len() > MAX_PAYLOAD {
        return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:pipe_payload_invalid"));
    }
    let mut frame = [0u8; 16];
    frame[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    frame[4..6].copy_from_slice(&VERSION.to_le_bytes());
    frame[6..8].copy_from_slice(&operation.to_le_bytes());
    frame[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    Ok(frame)
}

struct Event(*mut c_void);
impl Drop for Event {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: unique event handle from CreateEventW.
            unsafe { CloseHandle(self.0); }
        }
    }
}

fn remaining_ms(deadline: Instant) -> io::Result<u32> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::Error::new(ErrorKind::TimedOut, "native_capture:pipe_deadline"));
    }
    Ok(left.as_millis().min(u128::from(u32::MAX - 1)).max(1) as u32)
}

fn transfer(file: &File, data: *mut u8, len: usize,
            reading: bool, deadline: Instant) -> io::Result<usize> {
    if len == 0 { return Ok(0); }
    // Establish the timeout BEFORE submitting a Windows overlapped operation.
    // Once submitted, ALL failure paths must cancel/reap it before returning:
    // the buffer, event and stack OVERLAPPED must stay alive until completion.
    let timeout = remaining_ms(deadline)?;
    let event = Event(unsafe { CreateEventW(ptr::null_mut(), 1, 0, ptr::null()) });
    if event.0.is_null() { return Err(io::Error::last_os_error()); }
    let mut op = Overlapped { event: event.0, ..Overlapped::default() };
    let handle: *mut c_void = file.as_raw_handle().cast();
    let mut transferred = 0u32;
    let count = u32::try_from(len).map_err(|_| {
        io::Error::new(ErrorKind::InvalidInput, "native_capture:pipe_transfer_too_large")
    })?;
    // SAFETY: data points into a slice owned by the enclosing exact-transfer
    // loop. When ReadFile is pending, CancelIoEx+GetOverlappedResult wait
    // until the operation ceases before event/buffer scope ends.
    let ok = unsafe {
        if reading {
            ReadFile(handle, data.cast(), count, &mut transferred, &mut op)
        } else {
            WriteFile(handle, data.cast(), count, &mut transferred, &mut op)
        }
    };
    if ok == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_IO_PENDING) { return Err(error); }
        let wait = unsafe { WaitForSingleObject(event.0, timeout) };
        if wait != WAIT_OBJECT_0 {
            unsafe {
                CancelIoEx(handle, &mut op);
                let mut ignored = 0u32;
                GetOverlappedResult(handle, &mut op, &mut ignored, 1);
            }
            return Err(if wait == WAIT_TIMEOUT {
                io::Error::new(ErrorKind::TimedOut, "native_capture:pipe_deadline")
            } else {
                io::Error::new(ErrorKind::Other, "native_capture:pipe_wait_failed")
            });
        }
    }
    let completed = unsafe { GetOverlappedResult(handle, &mut op, &mut transferred, 0) };
    if completed == 0 { return Err(io::Error::last_os_error()); }
    if transferred == 0 || transferred > count {
        return Err(io::Error::new(ErrorKind::UnexpectedEof, "native_capture:pipe_closed"));
    }
    Ok(transferred as usize)
}

fn write_exact(file: &File, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let ptr = unsafe { bytes.as_ptr().add(offset) as *mut u8 };
        offset += transfer(file, ptr, bytes.len() - offset, false, deadline)?;
    }
    Ok(())
}
fn read_exact(file: &File, bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let ptr = unsafe { bytes.as_mut_ptr().add(offset) };
        offset += transfer(file, ptr, bytes.len() - offset, true, deadline)?;
    }
    Ok(())
}

fn transact(file: &File, operation: u16, payload: &[u8]) -> io::Result<Vec<u8>> {
    let header = request_bytes(operation, payload)?;
    let deadline = Instant::now() + IPC_TIMEOUT;
    write_exact(file, &header, deadline)?;
    write_exact(file, payload, deadline)?;
    let mut reply = [0u8; 16];
    read_exact(file, &mut reply, deadline)?;
    let version = u16::from_le_bytes(reply[4..6].try_into().unwrap());
    let op = u16::from_le_bytes(reply[6..8].try_into().unwrap());
    let status = u32::from_le_bytes(reply[8..12].try_into().unwrap());
    let body_bytes = u32::from_le_bytes(reply[12..16].try_into().unwrap());
    if u32::from_le_bytes(reply[0..4].try_into().unwrap()) != MAGIC
        || version != VERSION || op != operation
        || body_bytes > 40 || (operation != STATUS && body_bytes != 0)
    {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:broker_reply_invalid"));
    }
    if status != 0 {
        if body_bytes != 0 || status > i32::MAX as u32 {
            return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:broker_error_invalid"));
        }
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if operation == STATUS && body_bytes != 40 {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:broker_status_size_invalid"));
    }
    let mut body = vec![0u8; body_bytes as usize];
    read_exact(file, &mut body, deadline)?;
    Ok(body)
}

/// A single connection is one authenticated capture writer, not an AI owner.
pub struct NativeCapturePipeSession {
    file: File,
    generation: u64,
    source_epoch: u64,
    next_sequence: u64,
    active: bool,
}

impl NativeCapturePipeSession {
    pub fn begin(generation: u64, epoch: u64) -> io::Result<Self> {
        let bytes = begin_bytes(generation, epoch)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).access_mode(CLIENT_ACCESS)
            .custom_flags(FILE_FLAG_OVERLAPPED);
        let mut file = options.open(PIPE)?;
        verify_service_process(&file)?;
        transact(&mut file, BEGIN, &bytes)?;
        Ok(Self { file, generation, source_epoch: epoch, next_sequence: 1, active: true })
    }

    pub fn submit(&mut self, samples: &[i16]) -> io::Result<()> {
        if !self.active || self.next_sequence == u64::MAX {
            return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:invalid_writer_state"));
        }
        let bytes = frame_bytes(self.generation, self.source_epoch, self.next_sequence, samples)?;
        transact(&mut self.file, WRITE, &bytes)?;
        self.next_sequence += 1;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        if !self.active {
            return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:writer_not_active"));
        }
        let bytes = revoke_bytes(self.generation, self.source_epoch)?;
        transact(&mut self.file, FLUSH, &bytes)?;
        self.active = false;
        Ok(())
    }

    pub fn resume(&mut self, generation: u64, epoch: u64) -> io::Result<()> {
        if self.active || generation < self.generation
            || (generation == self.generation && epoch <= self.source_epoch)
        {
            return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:resume_epoch_invalid"));
        }
        let bytes = begin_bytes(generation, epoch)?;
        transact(&mut self.file, BEGIN, &bytes)?;
        self.generation = generation;
        self.source_epoch = epoch;
        self.next_sequence = 1;
        self.active = true;
        Ok(())
    }

    pub fn status(&mut self) -> io::Result<NativeCaptureStatus> {
        let reply = transact(&mut self.file, STATUS, &[])?;
        parse_status(&reply)
    }

    pub fn end(mut self) -> io::Result<()> {
        if self.active {
            let bytes = revoke_bytes(self.generation, self.source_epoch)?;
            transact(&mut self.file, END, &bytes)?;
            self.active = false;
        }
        // Closing the pipe immediately makes the broker close its driver
        // handle, which triggers kernel CLEANUP for the exact writer.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broker_protocol_has_bounded_little_endian_header() {
        let h = request_bytes(BEGIN, &begin_bytes(7, 2).unwrap()).unwrap();
        assert_eq!(h.len(), 16);
        assert_eq!(&h[..4], &MAGIC.to_le_bytes());
        assert_eq!(&h[4..6], &1u16.to_le_bytes());
        assert_eq!(&h[6..8], &BEGIN.to_le_bytes());
        assert_eq!(&h[8..12], &32u32.to_le_bytes());
        assert_eq!(&h[12..16], &[0, 0, 0, 0]);
        assert!(request_bytes(WRITE, &[0u8; MAX_PAYLOAD + 1]).is_err());
        assert!(request_bytes(0, &[]).is_err());
        assert_eq!(CLIENT_ACCESS & 0x0004, 0, "client cannot create pipe instance");
    }
}
