//! One process-wide Windows module identity, separate from resident model ownership.
use super::{Error, StopCheck, ABI};
use std::{
    ffi::{c_char, c_void, OsString},
    os::windows::{ffi::{OsStrExt, OsStringExt}, fs::MetadataExt},
    path::{Component, Path, PathBuf, Prefix},
    ptr::{self, NonNull},
    sync::Mutex,
};

type Version = unsafe extern "C" fn() -> u32;
type Open = unsafe extern "C" fn(*const c_char, *const c_char, *const c_char,
    *const c_char, *mut *mut c_void) -> i32;
type Close = unsafe extern "C" fn(*mut c_void);
type Transcribe = unsafe extern "C" fn(*mut c_void, *const f32, usize, u32,
    StopCheck, *const c_void, *mut c_char, usize, *mut usize) -> i32;
type Translate = unsafe extern "C" fn(*mut c_void, *const c_char, usize, u32, u32,
    StopCheck, *const c_void, *mut c_char, usize, *mut usize) -> i32;

#[derive(Clone, Copy)]
pub(super) struct Api {
    pub open: Open,
    pub close: Close,
    pub transcribe: Transcribe,
    pub translate: Translate,
}

const DLL_NAME: &str = "translateit_native_cuda.dll";
const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;
const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;
const GET_MODULE_HANDLE_EX_FLAG_PIN: u32 = 0x1;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x4;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, path: *mut u16, size: u32) -> u32;
    fn GetModuleHandleExW(flags: u32, name_or_address: *const u16, module: *mut *mut c_void) -> i32;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

struct Module(NonNull<c_void>);
impl Drop for Module {
    fn drop(&mut self) { unsafe { FreeLibrary(self.0.as_ptr()); } }
}
struct Loaded { path: PathBuf, api: Api }
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

fn controlled_path(path: &Path) -> Result<PathBuf, Error> {
    if !path.is_absolute() || path.file_name().and_then(|name| name.to_str()) != Some(DLL_NAME) {
        return Err(Error::LibraryIdentityMismatch);
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| Error::LibraryUnavailable)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Error::LibraryIdentityMismatch);
    }
    let path = std::fs::canonicalize(path).map_err(|_| Error::LibraryUnavailable)?;
    if !matches!(path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))) {
        return Err(Error::LibraryIdentityMismatch);
    }
    Ok(path)
}

/// Caller authenticates the DLL, dependency closure and controlled parent.
pub(super) unsafe fn load(path: &Path) -> Result<Api, Error> {
    let path = controlled_path(path)?;
    let mut slot = LOADED.lock().map_err(|_| Error::LibraryUnavailable)?;
    if let Some(loaded) = slot.as_ref() {
        return if loaded.path == path { Ok(loaded.api) } else { Err(Error::LibraryIdentityMismatch) };
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) { return Err(Error::LibraryIdentityMismatch); }
    wide.push(0);
    let module = Module(NonNull::new(unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(),
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) })
        .ok_or(Error::LibraryUnavailable)?);
    let mut actual = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(module.0.as_ptr(), actual.as_mut_ptr(), actual.len() as u32) } as usize;
    if len == 0 || len >= actual.len() { return Err(Error::LibraryIdentityMismatch); }
    let actual = PathBuf::from(OsString::from_wide(&actual[..len]));
    if std::fs::canonicalize(actual).ok().as_ref() != Some(&path) {
        return Err(Error::LibraryIdentityMismatch);
    }
    macro_rules! symbol {
        ($name:literal, $kind:ty) => {{
            let raw = unsafe { GetProcAddress(module.0.as_ptr(), concat!($name, "\0").as_ptr().cast()) };
            if raw.is_null() { return Err(Error::LibrarySymbolMissing); }
            unsafe { std::mem::transmute::<*mut c_void, $kind>(raw) }
        }};
    }
    let version = symbol!("ti_cuda_abi_version", Version);
    if unsafe { version() } != ABI { return Err(Error::AbiMismatch); }
    let api = Api {
        open: symbol!("ti_runtime_open", Open),
        close: symbol!("ti_runtime_close", Close),
        transcribe: symbol!("ti_runtime_transcribe", Transcribe),
        translate: symbol!("ti_runtime_translate", Translate),
    };
    // llama.cpp backend initialization is process-global. Pin the admitted
    // module; close each model pair on its own thread, not the DLL globals.
    let mut pinned = ptr::null_mut();
    if unsafe { GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN | GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
        version as *const () as *const u16, &mut pinned) } == 0 || pinned != module.0.as_ptr() {
        return Err(Error::LibraryIdentityMismatch);
    }
    *slot = Some(Loaded { path, api });
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_relative_and_wrong_library_names_before_loading() {
        assert_eq!(controlled_path(Path::new(DLL_NAME)), Err(Error::LibraryIdentityMismatch));
        assert_eq!(controlled_path(Path::new(r"C:\TranslateIT\other.dll")),
            Err(Error::LibraryIdentityMismatch));
    }
}
