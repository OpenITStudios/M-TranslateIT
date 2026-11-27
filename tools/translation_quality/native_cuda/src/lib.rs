//! Native CUDA binding; execution is admitted by the host.
#![cfg(windows)]
//! Reuse the application's SHA-256 dependency; no by-value llama.cpp ABI structs.
use std::{ffi::{c_char, c_void, CString}, marker::PhantomData, path::Path,
          ptr::NonNull, rc::Rc, sync::atomic::{AtomicBool, Ordering}, time::Instant};

mod loader;
mod owner;
pub use owner::{InferenceInput, PendingInference, RuntimeOwner};

const ABI: u32 = 5;
const CANCELLED: i32 = 16;
const DEADLINE: i32 = 17;
type StopCheck = unsafe extern "C" fn(*const c_void) -> i32;
const MAX_TRANSLATION_INPUT_CHARS: usize = 2_000;
const MAX_TRANSLATION_OUTPUT_CHARS: usize = 4_000;
const MAX_OUTPUT_BYTES: usize = 16_000;
const MIN_SAMPLES: usize = 4_800;
const MAX_SAMPLES: usize = 960_000;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language { Indonesian = 1, English = 2 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    AbiMismatch,
    LibraryUnavailable,
    LibraryIdentityMismatch,
    LibrarySymbolMissing,
    InvalidInput,
    Native(i32),
    InvalidReceipt,
    Cancelled,
    DeadlineExceeded,
    OwnerBusy,
    OwnerUnavailable,
    OwnerThreadFailed,
}

/// The caller supplies the existing cancellation flag and absolute deadline.
/// No timer thread, new queue or per-stage deadline renewal.
pub struct RequestControl<'a> {
    pub cancelled: &'a AtomicBool,
    pub deadline: Instant,
}

impl RequestControl<'_> {
    fn status(&self) -> i32 {
        if self.cancelled.load(Ordering::Acquire) { CANCELLED }
        else if Instant::now() >= self.deadline { DEADLINE }
        else { 0 }
    }
    fn data(&self) -> *const c_void { (self as *const Self).cast() }
}

// SAFETY: Rust passes a live RequestControl for the entire synchronous call.
// The native ABI does not retain the pointer, invoke asynchronously or mutate it.
unsafe extern "C" fn stop_check(data: *const c_void) -> i32 {
    if data.is_null() { return CANCELLED; }
    unsafe { &*data.cast::<RequestControl<'_>>() }.status()
}

pub struct Runtime {
    handle: NonNull<c_void>,
    api: loader::Api,
    device_id: String,
    // A single caller thread owns the mutable native context; do not
    // implicitly make this handle Send/Sync or add a second scheduler.
    _thread_bound: PhantomData<Rc<()>>,
}

fn valid_pci_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    let domain = match bytes.len() { 12 => 4, 16 => 8, _ => return false };
    bytes[domain] == b':' && bytes[domain + 3] == b':'
        && bytes[domain + 6] == b'.' && (b'0'..=b'7').contains(&bytes[domain + 7])
        && bytes[..bytes.len() - 1].iter().enumerate().all(|(i, &b)| {
            matches!(i, x if x == domain || x == domain + 3 || x == domain + 6)
                || b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
        })
}

fn native_path(path: &Path) -> Result<CString, Error> {
    if !path.is_absolute() { return Err(Error::InvalidInput); }
    CString::new(path.to_str().ok_or(Error::InvalidInput)?)
        .map_err(|_| Error::InvalidInput)
}

impl Runtime {
    /// One resident pair. The native owner validates all pinned artifacts
    /// and checks free VRAM before publication. No partial model opens.
    ///
    /// # Safety
    /// Supply reviewed build pins for the entire private DLL closure, a
    /// trusted parent and verified system dependency policy. Never trust
    /// hashes read from the installed DLL directory itself.
    pub unsafe fn open(dll: &Path, pins: &str, asr: &Path, projector: &Path, mt: &Path,
                       pci_id: Option<&str>) -> Result<Self, Error> {
        let api = unsafe { loader::load(dll, pins)? };
        let asr = native_path(asr)?;
        let projector = native_path(projector)?;
        let mt = native_path(mt)?;
        if pci_id == Some("") { return Err(Error::InvalidInput); }
        let device = CString::new(pci_id.unwrap_or_default()).map_err(|_| Error::InvalidInput)?;
        let mut raw = std::ptr::null_mut();
        let mut actual = [0u8; 32];
        let status = unsafe {
            (api.open)(asr.as_ptr(), projector.as_ptr(), mt.as_ptr(), device.as_ptr(),
                       actual.as_mut_ptr().cast(), actual.len(), &mut raw)
        };
        if status != 0 {
            // Native contract requires null on error; do not dereference an
            // unexpected pointer from an invalid receipt.
            return Err(if raw.is_null() && actual[0] == 0 { Error::Native(status) } else { Error::InvalidReceipt });
        }
        let handle = NonNull::new(raw).ok_or(Error::InvalidReceipt)?;
        let actual = actual.iter().position(|&byte| byte == 0)
            .and_then(|len| std::str::from_utf8(&actual[..len]).ok())
            .filter(|id| valid_pci_id(id) && pci_id.is_none_or(|expected| expected == *id));
        let Some(actual) = actual else {
            unsafe { (api.close)(handle.as_ptr()); }
            return Err(Error::InvalidReceipt);
        };
        Ok(Self { handle, api, device_id: actual.to_owned(), _thread_bound: PhantomData })
    }

    pub fn device_id(&self) -> &str { &self.device_id }

    /// 16 kHz mono finalized PCM. The caller owns the bounded reusable output
    /// scratch; return a borrowed transcript, not a second String allocation.
    pub fn transcribe<'a>(&mut self, pcm: &[f32], language: Language,
                          control: &RequestControl<'_>, output: &'a mut [u8]) -> Result<&'a str, Error> {
        if !(MIN_SAMPLES..=MAX_SAMPLES).contains(&pcm.len()) ||
            output.len() < MAX_OUTPUT_BYTES {
            return Err(Error::InvalidInput);
        }
        let mut len = 0;
        let status = unsafe {
            (self.api.transcribe)(self.handle.as_ptr(), pcm.as_ptr(), pcm.len(),
                language as u32, stop_check, control.data(), output.as_mut_ptr().cast(), output.len(), &mut len)
        };
        let text = checked_output(status, len, output, usize::MAX)?;
        check_control(control)?;
        Ok(text)
    }

    /// Translate only the current finalized source, returning a borrowed
    /// complete result. No external prompt, context, or generation options.
    pub fn translate<'a>(&mut self, source: &str, from: Language, to: Language,
                         control: &RequestControl<'_>, output: &'a mut [u8]) -> Result<&'a str, Error> {
        if from == to || source.trim().is_empty() || source.contains('\0') ||
            source.chars().take(MAX_TRANSLATION_INPUT_CHARS + 1).count() > MAX_TRANSLATION_INPUT_CHARS ||
            output.len() < MAX_OUTPUT_BYTES {
            return Err(Error::InvalidInput);
        }
        let mut len = 0;
        let status = unsafe {
            (self.api.translate)(self.handle.as_ptr(), source.as_ptr().cast(), source.len(),
                from as u32, to as u32, stop_check, control.data(),
                output.as_mut_ptr().cast(), output.len(), &mut len)
        };
        let text = checked_output(status, len, output, MAX_TRANSLATION_OUTPUT_CHARS)?;
        check_control(control)?;
        Ok(text)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) { unsafe { (self.api.close)(self.handle.as_ptr()); } }
}

fn check_control(control: &RequestControl<'_>) -> Result<(), Error> {
    match control.status() {
        0 => Ok(()),
        CANCELLED => Err(Error::Cancelled),
        _ => Err(Error::DeadlineExceeded),
    }
}

fn checked_output(status: i32, len: usize, output: &[u8], max_chars: usize)
    -> Result<&str, Error> {
    if status != 0 {
        return Err(if len != 0 { Error::InvalidReceipt } else { match status {
            CANCELLED => Error::Cancelled,
            DEADLINE => Error::DeadlineExceeded,
            _ => Error::Native(status),
        }});
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

#[cfg(test)]
mod tests {
    use super::*;

    // Held, explicitly selected Windows integrated acceptance only. The
    // fixture must not report PASS if actual model paths/device are absent.
    #[test]
    #[ignore = "requires authorized target Windows GPU and exact pinned assets"]
    fn resident_pair_rejects_wrong_assets_and_duplicate_open_then_releases_lease() {
        let required = |key| std::env::var(key).expect("missing target acceptance input");
        let dll = std::path::PathBuf::from(required("TRANSLATEIT_TEST_NATIVE_DLL"));
        let pins = required("TRANSLATEIT_TEST_NATIVE_DLL_PINS");
        let asr = std::path::PathBuf::from(required("TRANSLATEIT_TEST_ASR_GGUF"));
        let projector = std::path::PathBuf::from(required("TRANSLATEIT_TEST_PROJECTOR_GGUF"));
        let mt = std::path::PathBuf::from(required("TRANSLATEIT_TEST_MT_GGUF"));
        let pci = required("TRANSLATEIT_TEST_CUDA_PCI");
        // Swap actual pinned files: the wrong projector must fail the exact
        // identity check before either model loads, and release the lease.
        assert!(matches!(unsafe { Runtime::open(&dll, &pins, &asr, &asr, &mt, Some(&pci)) }, Err(Error::Native(13))));
        let mut pair = unsafe { Runtime::open(&dll, &pins, &asr, &projector, &mt, Some(&pci)) }.expect("candidate resident pair");
        assert_eq!(pair.device_id(), pci.as_str());
        let cancelled = AtomicBool::new(true);
        let mut output = vec![0; MAX_OUTPUT_BYTES];
        let pcm = vec![0.0; MIN_SAMPLES];
        let control = RequestControl { cancelled: &cancelled,
            deadline: Instant::now() + std::time::Duration::from_secs(30) };
        assert_eq!(pair.transcribe(&pcm, Language::English, &control, &mut output),
                   Err(Error::Cancelled));
        assert_eq!(pair.translate("Halo", Language::Indonesian, Language::English, &control, &mut output),
                   Err(Error::Cancelled));
        cancelled.store(false, Ordering::Release);
        let expired = RequestControl { cancelled: &cancelled, deadline: Instant::now() };
        assert_eq!(pair.translate("Halo", Language::Indonesian, Language::English, &expired, &mut output),
                   Err(Error::DeadlineExceeded));
        // Stop during MT generation or prefill, not merely before dispatch.
        // The third callback is inside native inference; reject any partial receipt.
        unsafe extern "C" fn stop_during(data: *const c_void) -> i32 {
            let calls = unsafe { &*data.cast::<std::sync::atomic::AtomicUsize>() };
            if calls.fetch_add(1, Ordering::Relaxed) >= 2 { CANCELLED } else { 0 }
        }
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut len = 99;
        let status = unsafe {
            (pair.api.translate)(pair.handle.as_ptr(), b"Halo".as_ptr().cast(), 4,
                Language::Indonesian as u32, Language::English as u32,
                stop_during, (&calls as *const std::sync::atomic::AtomicUsize).cast(),
                output.as_mut_ptr().cast(), output.len(), &mut len)
        };
        assert_eq!((status, len), (CANCELLED, 0));
        assert!(calls.load(Ordering::Relaxed) >= 3);
        let active = RequestControl { cancelled: &cancelled,
            deadline: Instant::now() + std::time::Duration::from_secs(30) };
        assert!(!pair.translate("Halo", Language::Indonesian, Language::English,
                               &active, &mut output).expect("reuse after cooperative stop").is_empty());
        assert!(matches!(unsafe { Runtime::open(&dll, &pins, &asr, &projector, &mt, Some(&pci)) }, Err(Error::Native(15))));
        drop(pair);
        let mut reopened = unsafe { RuntimeOwner::open(&dll, &pins, &asr, &projector, &mt, None) }.expect("open on owner thread");
        assert_eq!(reopened.device_id(), pci.as_str(), "single visible CUDA identity");
        let pending = reopened.begin(InferenceInput::Translation {
            source: "Halo".into(), from: Language::Indonesian, to: Language::English,
        }, Instant::now() + std::time::Duration::from_secs(30)).expect("admitted native request");
        assert!(!pending.wait().expect("owned native translation").is_empty());
        // Drop joins the idle owner. Production Stop instead retains any
        // incomplete try_close result and does not block the UI.
        drop(reopened);
        let reopened = unsafe { Runtime::open(&dll, &pins, &asr, &projector, &mt, Some(&pci)) }.expect("lease released after owner close");
        drop(reopened);
    }

    #[test]
    fn pci_identity_receipt_is_exact_and_physical() {
        assert!(valid_pci_id("0000:01:00.0"));
        assert!(valid_pci_id("00000000:af:00.0"));
        for invalid in ["", "cuda:0", "0000:01:00.0-v0", "0000:AF:00.0", "0000:01:00.8"] {
            assert!(!valid_pci_id(invalid));
        }
    }

    #[test]
    fn output_receipt_never_exposes_failed_or_invalid_bytes() {
        assert_eq!(checked_output(7, 0, b"partial", 4000), Err(Error::Native(7)));
        assert_eq!(checked_output(CANCELLED, 0, b"partial", 4000), Err(Error::Cancelled));
        assert_eq!(checked_output(DEADLINE, 0, b"partial", 4000), Err(Error::DeadlineExceeded));
        assert_eq!(checked_output(CANCELLED, 7, b"partial", 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(7, 7, b"partial", 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 2, &[0xff, 0xfe], 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 4, b"test", 3), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 5, b"halo", 4000), Err(Error::InvalidReceipt));
        assert_eq!(checked_output(0, 6, b" halo ", 4000), Ok("halo"));
        assert_eq!(checked_output(0, 4, b"\0abc", 4000), Err(Error::InvalidReceipt));
    }
}
