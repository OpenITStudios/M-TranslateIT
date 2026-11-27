//! Source-only candidate, not linked into the production application.
//! No third-party Rust dependencies or by-value llama.cpp ABI structs.
use std::{ffi::{c_char, c_void, CString}, marker::PhantomData, path::Path,
          ptr::NonNull, rc::Rc};

const ABI: u32 = 1;
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
        if status != 0 {
            return Err(if len == 0 { Error::Native(status) } else { Error::InvalidReceipt });
        }
        if len == 0 || len > MAX_OUTPUT_BYTES || len > output.len() {
            return Err(Error::InvalidReceipt);
        }
        let text = std::str::from_utf8(&output[..len]).map_err(|_| Error::InvalidReceipt)?;
        let text = text.trim();
        if text.is_empty() || text.contains('\0') { return Err(Error::InvalidReceipt); }
        Ok(text)
    }
}

impl Drop for Asr {
    fn drop(&mut self) { unsafe { ti_asr_close(self.handle.as_ptr()); } }
}
