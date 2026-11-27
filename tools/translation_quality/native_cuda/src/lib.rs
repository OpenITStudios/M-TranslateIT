//! Source-only candidate, not linked into the production application.
//! No third-party Rust dependencies or by-value llama.cpp ABI structs.
use std::{ffi::{c_char, c_void, CString}, marker::PhantomData, path::Path,
          ptr::NonNull, rc::Rc};

const ABI: u32 = 2;
const MAX_TRANSLATION_INPUT_CHARS: usize = 2_000;
const MAX_TRANSLATION_OUTPUT_CHARS: usize = 4_000;
const MAX_OUTPUT_BYTES: usize = 16_000;
const MIN_SAMPLES: usize = 4_800;
const MAX_SAMPLES: usize = 960_000;

extern "C" {
    fn ti_cuda_abi_version() -> u32;
    fn ti_asr_open(model_path: *const c_char, projector_path: *const c_char,
                   device_id: *const c_char, out: *mut *mut c_void) -> i32;
    fn ti_asr_close(asr: *mut c_void);
    fn ti_asr_transcribe(asr: *mut c_void, pcm: *const f32, count: usize,
                         language: u32, output: *mut c_char, output_cap: usize,
                         out_len: *mut usize) -> i32;
    fn ti_mt_open(model_path: *const c_char, device_id: *const c_char,
                  out: *mut *mut c_void) -> i32;
    fn ti_mt_close(mt: *mut c_void);
    fn ti_mt_translate(mt: *mut c_void, source: *const c_char, source_len: usize,
                       source_language: u32, target_language: u32,
                       output: *mut c_char, output_cap: usize, out_len: *mut usize) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language { Indonesian = 1, English = 2 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    AbiMismatch,
    InvalidInput,
    Native(i32),
    InvalidReceipt,
}

pub struct Asr {
    handle: NonNull<c_void>,
    // A single caller thread owns the mutable native context; do not
    // implicitly make this handle Send/Sync or add a second scheduler.
    _thread_bound: PhantomData<Rc<()>>,
}

fn native_path(path: &Path) -> Result<CString, Error> {
    if !path.is_absolute() { return Err(Error::InvalidInput); }
    CString::new(path.to_str().ok_or(Error::InvalidInput)?)
        .map_err(|_| Error::InvalidInput)
}

impl Asr {
    /// Caller must verify exact model/projector bytes before use. This
    /// developer candidate is not a production artifact-admission owner.
    pub fn open(model: &Path, projector: &Path, pci_id: &str) -> Result<Self, Error> {
        if unsafe { ti_cuda_abi_version() } != ABI { return Err(Error::AbiMismatch); }
        let model = native_path(model)?;
        let projector = native_path(projector)?;
        if pci_id.is_empty() { return Err(Error::InvalidInput); }
        let device = CString::new(pci_id).map_err(|_| Error::InvalidInput)?;
        let mut raw = std::ptr::null_mut();
        let status = unsafe {
            ti_asr_open(model.as_ptr(), projector.as_ptr(), device.as_ptr(), &mut raw)
        };
        if status != 0 {
            // Native contract requires null on error; do not dereference an
            // unexpected pointer from an invalid receipt.
            return Err(if raw.is_null() { Error::Native(status) } else { Error::InvalidReceipt });
        }
        let handle = NonNull::new(raw).ok_or(Error::InvalidReceipt)?;
        Ok(Self { handle, _thread_bound: PhantomData })
    }

    /// 16 kHz mono finalized PCM. The caller owns the bounded reusable output
    /// scratch; return a borrowed transcript, not a second String allocation.
    pub fn transcribe<'a>(&mut self, pcm: &[f32], language: Language,
                          output: &'a mut [u8]) -> Result<&'a str, Error> {
        if !(MIN_SAMPLES..=MAX_SAMPLES).contains(&pcm.len()) ||
            output.len() < MAX_OUTPUT_BYTES {
            return Err(Error::InvalidInput);
        }
        let mut len = 0;
        let status = unsafe {
            ti_asr_transcribe(self.handle.as_ptr(), pcm.as_ptr(), pcm.len(),
                language as u32, output.as_mut_ptr().cast(), output.len(), &mut len)
        };
        checked_output(status, len, output, usize::MAX)
    }
}

impl Drop for Asr {
    fn drop(&mut self) { unsafe { ti_asr_close(self.handle.as_ptr()); } }
}

fn checked_output(status: i32, len: usize, output: &[u8], max_chars: usize)
    -> Result<&str, Error> {
    if status != 0 {
        return Err(if len == 0 { Error::Native(status) } else { Error::InvalidReceipt });
    }
    if len == 0 || len > MAX_OUTPUT_BYTES || len > output.len() {
        return Err(Error::InvalidReceipt);
    }
    let text = std::str::from_utf8(&output[..len]).map_err(|_| Error::InvalidReceipt)?;
    let text = text.trim();
    if text.is_empty() || text.contains('\0') ||
        (max_chars != usize::MAX && text.chars().take(max_chars + 1).count() > max_chars) {
        return Err(Error::InvalidReceipt);
    }
    Ok(text)
}

pub struct Translation {
    handle: NonNull<c_void>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Translation {
    /// Caller verifies pinned GGUF bytes. Both directions use this one model.
    pub fn open(model: &Path, pci_id: &str) -> Result<Self, Error> {
        if unsafe { ti_cuda_abi_version() } != ABI { return Err(Error::AbiMismatch); }
        let model = native_path(model)?;
        if pci_id.is_empty() { return Err(Error::InvalidInput); }
        let device = CString::new(pci_id).map_err(|_| Error::InvalidInput)?;
        let mut raw = std::ptr::null_mut();
        let status = unsafe { ti_mt_open(model.as_ptr(), device.as_ptr(), &mut raw) };
        if status != 0 {
            return Err(if raw.is_null() { Error::Native(status) } else { Error::InvalidReceipt });
        }
        let handle = NonNull::new(raw).ok_or(Error::InvalidReceipt)?;
        Ok(Self { handle, _thread_bound: PhantomData })
    }

    /// Translate only the current finalized source, returning a borrowed
    /// complete result. No external prompt, context, or generation options.
    pub fn translate<'a>(&mut self, source: &str, from: Language, to: Language,
                         output: &'a mut [u8]) -> Result<&'a str, Error> {
        if from == to || source.trim().is_empty() || source.contains('\0') ||
            source.chars().take(MAX_TRANSLATION_INPUT_CHARS + 1).count() > MAX_TRANSLATION_INPUT_CHARS ||
            output.len() < MAX_OUTPUT_BYTES {
            return Err(Error::InvalidInput);
        }
        let mut len = 0;
        let status = unsafe {
            ti_mt_translate(self.handle.as_ptr(), source.as_ptr().cast(), source.len(),
                from as u32, to as u32, output.as_mut_ptr().cast(), output.len(), &mut len)
        };
        checked_output(status, len, output, MAX_TRANSLATION_OUTPUT_CHARS)
    }
}

impl Drop for Translation {
    fn drop(&mut self) { unsafe { ti_mt_close(self.handle.as_ptr()); } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_receipt_never_exposes_failed_or_invalid_bytes() {
        assert_eq!(checked_output(7, 0, b"partial", 4000), Err(Error::Native(7)));
        assert_eq!(checked_output(7, 7, b"partial", 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 2, &[0xff, 0xfe], 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 4, b"test", 3), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 5, b"halo", 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 6, b" halo ", 4000), Ok("halo"));
        assert_eq!(checked_output(0, 4, b"\0abc", 4000), Err(Error::InvalidReceipt));
    }
}
