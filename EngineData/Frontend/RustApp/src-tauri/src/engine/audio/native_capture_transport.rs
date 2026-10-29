//! Privileged capture-only PCM transport, not a second Meeting output owner.
//! This module is compiled on Windows, but production route admission stays
//! disabled until driver, broker security and real Windows acceptance succeed.
//!
//! One Rust Meeting owner may hold one NativeCaptureSession. No file or audio
//! buffer survives normal Drop: kernel IRP_MJ_CLEANUP revokes any owned frames.
//! Regular non-elevated users cannot open the current admin/SYSTEM-only device.
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::windows::io::AsRawHandle;
use std::ptr;

pub const SAMPLE_RATE_HZ: u32 = 48_000;
pub const CHANNELS: u32 = 1;
pub const SAMPLE_BITS: u32 = 16;
pub const MAX_FRAMES: usize = 960;
const VERSION: u32 = 1;
const DEVICE_PATH: &str = r"\\.\TranslateITMicControl";
const DEVICE_TYPE: u32 = 0x22;
const FILE_READ_DATA: u32 = 1;
const FILE_WRITE_DATA: u32 = 2;
const METHOD_BUFFERED: u32 = 0;

const fn control_code(function: u32, access: u32) -> u32 {
    (DEVICE_TYPE << 16) | (access << 14) | (function << 2) | METHOD_BUFFERED
}
const IOCTL_BEGIN: u32 = control_code(0x900, FILE_WRITE_DATA);
const IOCTL_WRITE: u32 = control_code(0x901, FILE_WRITE_DATA);
const IOCTL_FLUSH: u32 = control_code(0x902, FILE_WRITE_DATA);
const IOCTL_END: u32 = control_code(0x903, FILE_WRITE_DATA);
const IOCTL_STATUS: u32 = control_code(0x904, FILE_READ_DATA);

#[link(name = "kernel32")]
extern "system" {
    fn DeviceIoControl(
        device: *mut c_void,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, reason)
}

fn send_control(file: &File, code: u32, input: &[u8], output: &mut [u8]) -> io::Result<usize> {
    let input_len = u32::try_from(input.len()).map_err(|_| invalid("native_capture:input_too_large"))?;
    let output_len = u32::try_from(output.len()).map_err(|_| invalid("native_capture:output_too_large"))?;
    let mut returned = 0u32;
    // SAFETY: file owns a valid kernel handle while this synchronous IOCTL
    // executes; buffers remain allocated, checked and borrowed until return.
    // Windows METHOD_BUFFERED copies request/reply into its SYSTEM_BUFFER.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as *mut c_void,
            code,
            if input.is_empty() { ptr::null() } else { input.as_ptr().cast() },
            input_len,
            if output.is_empty() { ptr::null_mut() } else { output.as_mut_ptr().cast() },
            output_len,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let received = usize::try_from(returned).map_err(|_| invalid("native_capture:response_length_overflow"))?;
    if received > output.len() {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:response_length_invalid"));
    }
    Ok(received)
}

pub(super) fn begin_bytes(generation: u64, epoch: u64) -> io::Result<[u8; 32]> {
    if generation == 0 || epoch == 0 {
        return Err(invalid("native_capture:missing_session_authority"));
    }
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&VERSION.to_le_bytes());
    bytes[4..8].copy_from_slice(&SAMPLE_RATE_HZ.to_le_bytes());
    bytes[8..12].copy_from_slice(&CHANNELS.to_le_bytes());
    bytes[12..16].copy_from_slice(&SAMPLE_BITS.to_le_bytes());
    bytes[16..24].copy_from_slice(&generation.to_le_bytes());
    bytes[24..32].copy_from_slice(&epoch.to_le_bytes());
    Ok(bytes)
}

pub(super) fn revoke_bytes(generation: u64, epoch: u64) -> io::Result<[u8; 24]> {
    if generation == 0 || epoch == 0 {
        return Err(invalid("native_capture:missing_session_authority"));
    }
    let mut bytes = [0u8; 24];
    bytes[0..4].copy_from_slice(&VERSION.to_le_bytes());
    // bytes[4..8] is the required-zero reserved field.
    bytes[8..16].copy_from_slice(&generation.to_le_bytes());
    bytes[16..24].copy_from_slice(&epoch.to_le_bytes());
    Ok(bytes)
}

pub(super) fn frame_bytes(generation: u64, epoch: u64, sequence: u64, samples: &[i16]) -> io::Result<Vec<u8>> {
    if generation == 0 || epoch == 0 || sequence == 0 {
        return Err(invalid("native_capture:missing_frame_identity"));
    }
    if samples.is_empty() || samples.len() > MAX_FRAMES {
        return Err(invalid("native_capture:frame_size_invalid"));
    }
    let payload_bytes = u32::try_from(samples.len() * 2)
        .map_err(|_| invalid("native_capture:frame_size_overflow"))?;
    let mut packet = Vec::with_capacity(32 + payload_bytes as usize);
    packet.extend_from_slice(&VERSION.to_le_bytes());
    packet.extend_from_slice(&payload_bytes.to_le_bytes());
    packet.extend_from_slice(&generation.to_le_bytes());
    packet.extend_from_slice(&epoch.to_le_bytes());
    packet.extend_from_slice(&sequence.to_le_bytes());
    for sample in samples {
        packet.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(packet)
}

/// Owned by the existing Meeting output lifecycle, not a background service.
/// A second writer's BEGIN is rejected in kernel even when two handles open.
pub struct NativeCaptureSession {
    file: File,
    generation: u64,
    source_epoch: u64,
    next_sequence: u64,
    active: bool,
}

impl NativeCaptureSession {
    /// Only for a separately authorized, installed driver candidate.
    /// This is intentionally never selected by the current VB-CABLE route.
    pub fn begin(generation: u64, source_epoch: u64) -> io::Result<Self> {
        let packet = begin_bytes(generation, source_epoch)?;
        let file = OpenOptions::new().read(true).write(true).open(DEVICE_PATH)?;
        send_control(&file, IOCTL_BEGIN, &packet, &mut [])?;
        Ok(Self { file, generation, source_epoch, next_sequence: 1, active: true })
    }

    /// Submit at most 20ms of already-resampled PCM16 mono, from a non-audio
    /// callback thread. This does not schedule/playback or perform resampling.
    pub fn submit_20ms(&mut self, samples: &[i16]) -> io::Result<()> {
        if !self.active {
            return Err(invalid("native_capture:session_not_active"));
        }
        if self.next_sequence == u64::MAX {
            return Err(invalid("native_capture:sequence_exhausted"));
        }
        let packet = frame_bytes(
            self.generation, self.source_epoch, self.next_sequence, samples
        )?;
        send_control(&self.file, IOCTL_WRITE, &packet, &mut [])?;
        self.next_sequence += 1;
        Ok(())
    }

    /// Mute/source switch: kernel flush must ACK before the owner can report
    /// success. Old generation/epoch cannot write again after this call.
    pub fn flush(&mut self) -> io::Result<()> {
        if !self.active {
            return Err(invalid("native_capture:session_not_active"));
        }
        let packet = revoke_bytes(self.generation, self.source_epoch)?;
        send_control(&self.file, IOCTL_FLUSH, &packet, &mut [])?;
        self.active = false;
        Ok(())
    }

    /// Explicit resume requires an advanced source epoch. Never re-admit
    /// already queued samples as part of Unmute.
    pub fn resume(&mut self, generation: u64, new_epoch: u64) -> io::Result<()> {
        if self.active || generation < self.generation
            || (generation == self.generation && new_epoch <= self.source_epoch)
        {
            return Err(invalid("native_capture:invalid_resume_epoch"));
        }
        let packet = begin_bytes(generation, new_epoch)?;
        send_control(&self.file, IOCTL_BEGIN, &packet, &mut [])?;
        self.generation = generation;
        self.source_epoch = new_epoch;
        self.next_sequence = 1;
        self.active = true;
        Ok(())
    }

    /// Final Stop explicitly releases the writer. If this fails, the caller
    /// must retain/close the owner and must NOT claim that Stop succeeded.
    pub fn end(mut self) -> io::Result<()> {
        // Consuming self also closes the control handle after a prior FLUSH;
        // its kernel CLEANUP revokes the reserved single-writer lease.
        if self.active {
            let packet = revoke_bytes(self.generation, self.source_epoch)?;
            send_control(&self.file, IOCTL_END, &packet, &mut [])?;
            self.active = false;
        }
        // File::drop also invokes the kernel CLEANUP fail-safe if needed.
        Ok(())
    }

    /// Aggregate non-sensitive driver counters; never logs meeting PCM.
    pub fn status(&self) -> io::Result<NativeCaptureStatus> {
        let mut bytes = [0u8; 40];
        let received = send_control(&self.file, IOCTL_STATUS, &[], &mut bytes)?;
        if received != bytes.len() {
            return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:status_size_mismatch"));
        }
        parse_status(&bytes)

    }
}

pub(super) fn parse_status(bytes: &[u8]) -> io::Result<NativeCaptureStatus> {
    if bytes.len() != 40 {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:status_size_mismatch"));
    }
    let read_u32 = |offset: usize| -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("bounded status"))
    };
    let read_u64 = |offset: usize| -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("bounded status"))
    };
    if read_u32(0) != VERSION || read_u32(8) != 9_600 {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:status_contract_mismatch"));
    }
    let capacity = read_u32(8);
    let buffered = read_u32(12);
    if buffered > capacity || read_u32(4) > 1 {
        return Err(io::Error::new(ErrorKind::InvalidData, "native_capture:status_values_invalid"));
    }
    Ok(NativeCaptureStatus {
        producer_active: read_u32(4) != 0,
        buffered_frames: buffered,
        accepted_frames: read_u64(16),
        overwritten_frames: read_u64(24),
        underrun_frames: read_u64(32),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCaptureStatus {
    pub producer_active: bool,
    pub buffered_frames: u32,
    pub accepted_frames: u64,
    pub overwritten_frames: u64,
    pub underrun_frames: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ioctls_match_kernel_method_buffered_file_access() {
        assert_eq!(IOCTL_BEGIN, 0x22a400);
        assert_eq!(IOCTL_WRITE, 0x22a404);
        assert_eq!(IOCTL_FLUSH, 0x22a408);
        assert_eq!(IOCTL_END, 0x22a40c);
        assert_eq!(IOCTL_STATUS, 0x226410);
    }

    #[test]
    fn packet_abi_bounds_and_layout_are_stable() {
        let begin = begin_bytes(42, 9).unwrap();
        assert_eq!(begin.len(), 32);
        assert_eq!(&begin[4..8], &48_000u32.to_le_bytes());
        assert_eq!(&begin[16..24], &42u64.to_le_bytes());
        assert_eq!(&begin[24..32], &9u64.to_le_bytes());
        let revoke = revoke_bytes(42, 9).unwrap();
        assert_eq!(revoke.len(), 24);
        assert_eq!(&revoke[4..8], &[0, 0, 0, 0]);
        let pcm = frame_bytes(42, 9, 1, &[-32768, 0, 32767]).unwrap();
        assert_eq!(pcm.len(), 38);
        assert_eq!(&pcm[4..8], &6u32.to_le_bytes());
        assert_eq!(&pcm[32..38], &[0, 128, 0, 0, 255, 127]);
        assert!(frame_bytes(42, 9, 1, &[0; MAX_FRAMES + 1]).is_err());
        assert!(frame_bytes(42, 9, 0, &[0]).is_err());
        assert!(begin_bytes(0, 1).is_err());
    }
}
