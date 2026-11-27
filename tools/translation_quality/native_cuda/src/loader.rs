//! One process-wide Windows module identity, separate from resident model ownership.
use super::{Error, StopCheck, ABI};
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_char, c_void, OsString},
    fs::{File, OpenOptions},
    io::Read,
    os::windows::{ffi::{OsStrExt, OsStringExt}, fs::{MetadataExt, OpenOptionsExt}},
    path::{Component, Path, PathBuf, Prefix},
    ptr::{self, NonNull},
    sync::Mutex,
};

type Version = unsafe extern "C" fn() -> u32;
type Open = unsafe extern "C" fn(*const c_char, *const c_char, *const c_char,
    *const c_char, *mut c_char, usize, *mut *mut c_void) -> i32;
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
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
const FILE_READ_ATTRIBUTES: u32 = 0x80;
const FILE_SHARE_WRITE: u32 = 2;
const FILE_SHARE_READ: u32 = 1;
const MAX_DLL_BYTES: u64 = 1024 * 1024 * 1024;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, path: *mut u16, size: u32) -> u32;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleHandleExW(flags: u32, name_or_address: *const u16, module: *mut *mut c_void) -> i32;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

struct Module(NonNull<c_void>);
impl Drop for Module {
    fn drop(&mut self) { unsafe { FreeLibrary(self.0.as_ptr()); } }
}
struct Loaded {
    path: PathBuf,
    pins: String,
    api: Api,
    // Pin the verified closure's read handles with the DLL, not the model
    // pair. Models may reopen; executable files may not change in-process.
    _files: Vec<File>,
}
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

// ProjectPaths reports forward slashes. Convert at the Windows ABI boundary
// before checking disk prefixes, opening handles or passing paths to C++.
// Do not canonicalize away a redirected parent or a parent traversal.
pub(super) fn dos_path(path: &Path) -> Result<PathBuf, Error> {
    let path = PathBuf::from(path.to_str().ok_or(Error::LibraryIdentityMismatch)?.replace('/', "\\"));
    if !path.is_absolute()
        || !matches!(path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || path.components().any(|component| matches!(component, Component::ParentDir))
    {
        return Err(Error::LibraryIdentityMismatch);
    }
    Ok(path)
}

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

// Lock directory names root-to-leaf while resolving and loading the exact
// private closure. This rejects directory junctions/reparse points before
// canonicalization can hide them. It does not establish directory ACL trust.
pub(super) fn lock_parents(path: &Path) -> Result<Vec<File>, Error> {
    if !path.is_absolute()
        || !matches!(path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || path.components().any(|component| matches!(component, Component::ParentDir))
    { return Err(Error::LibraryIdentityMismatch); }
    let mut parents: Vec<&Path> = path.parent().ok_or(Error::LibraryIdentityMismatch)?
        .ancestors().filter(|parent| parent.parent().is_some() && parent.has_root()).collect();
    parents.reverse();
    let mut locked = Vec::with_capacity(parents.len());
    for parent in parents {
        let file = OpenOptions::new().access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(parent).map_err(|_| Error::LibraryUnavailable)?;
        let meta = file.metadata().map_err(|_| Error::LibraryUnavailable)?;
        if !meta.is_dir() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::LibraryIdentityMismatch);
        }
        locked.push(file);
    }
    Ok(locked)
}

// The caller supplies pins from a trusted build review, not an installed
// manifest or hashes calculated from whatever files happen to be present.
fn parse_pins(pins: &str) -> Result<Vec<(&str, &str)>, Error> {
    if pins.len() > 8192 || pins.is_empty() { return Err(Error::LibraryIdentityMismatch); }
    let mut result = Vec::new();
    for entry in pins.split(';') {
        let (name, hash) = entry.split_once('=').ok_or(Error::LibraryIdentityMismatch)?;
        if name.len() > 100 || !name.ends_with(".dll")
            || !name.as_bytes()[0].is_ascii_alphanumeric()
            || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-' || b == b'.')
            || name.contains("..")
            || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || result.iter().any(|(previous, _)| *previous == name)
        { return Err(Error::LibraryIdentityMismatch); }
        result.push((name, hash));
        if result.len() > 32 { return Err(Error::LibraryIdentityMismatch); }
    }
    if !result.iter().any(|(name, _)| *name == DLL_NAME) {
        return Err(Error::LibraryIdentityMismatch);
    }
    Ok(result)
}

fn module_path(module: *mut c_void) -> Result<PathBuf, Error> {
    let mut actual = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(module, actual.as_mut_ptr(), actual.len() as u32) } as usize;
    if len == 0 || len >= actual.len() { return Err(Error::LibraryIdentityMismatch); }
    std::fs::canonicalize(PathBuf::from(OsString::from_wide(&actual[..len])))
        .map_err(|_| Error::LibraryIdentityMismatch)
}

fn verify_closure(directory: &Path, pins: &str) -> Result<Vec<File>, Error> {
    let entries = parse_pins(pins)?;
    // A private DLL directory is flat and closed: no extra private
    // dependency, symlink/junction or unreviewed sidecar is admitted.
    let mut count = 0;
    for entry in std::fs::read_dir(directory).map_err(|_| Error::LibraryUnavailable)? {
        let entry = entry.map_err(|_| Error::LibraryUnavailable)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::LibraryIdentityMismatch)?;
        if !entries.iter().any(|(expected, _)| *expected == name) {
            return Err(Error::LibraryIdentityMismatch);
        }
        count += 1;
    }
    if count != entries.len() { return Err(Error::LibraryUnavailable); }
    let mut locked = Vec::with_capacity(entries.len());
    let mut scratch = vec![0u8; 1024 * 1024];
    for (name, expected) in entries {
        let path = directory.join(name);
        let mut file = OpenOptions::new().read(true)
            .share_mode(FILE_SHARE_READ).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path).map_err(|_| Error::LibraryUnavailable)?;
        let metadata = file.metadata().map_err(|_| Error::LibraryUnavailable)?;
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || metadata.len() == 0 || metadata.len() > MAX_DLL_BYTES
            || std::fs::canonicalize(&path).ok().as_ref() != Some(&path)
        { return Err(Error::LibraryIdentityMismatch); }
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        loop {
            let read = file.read(&mut scratch).map_err(|_| Error::LibraryUnavailable)?;
            if read == 0 { break; }
            bytes += read as u64;
            if bytes > metadata.len() { return Err(Error::LibraryIdentityMismatch); }
            hash.update(&scratch[..read]);
        }
        let actual = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        if bytes != metadata.len() || actual != expected {
            return Err(Error::LibraryIdentityMismatch);
        }
        // Before first admission, even a same-path preloaded image is not
        // bound to the freshly verified file bytes. Subsequent opens use
        // the process-pinned LOADED receipt instead of reloading.
        let wide: Vec<u16> = OsString::from(name).encode_wide().chain(Some(0)).collect();
        let existing = unsafe { GetModuleHandleW(wide.as_ptr()) };
        if !existing.is_null() {
            return Err(Error::LibraryIdentityMismatch);
        }
        locked.push(file);
    }
    Ok(locked)
}

/// Caller supplies reviewed build pins and a trusted, controlled parent.
/// OS/system dependencies and the actual PE import closure remain release
/// evidence requirements; pinning a list is not a PE dependency audit.
pub(super) unsafe fn load(path: &Path, pins: &str) -> Result<Api, Error> {
    let path = dos_path(path)?;
    let _parents = lock_parents(&path)?;
    let path = controlled_path(&path)?;
    let mut slot = LOADED.lock().map_err(|_| Error::LibraryUnavailable)?;
    if let Some(loaded) = slot.as_ref() {
        return if loaded.path == path && loaded.pins == pins {
            Ok(loaded.api)
        } else { Err(Error::LibraryIdentityMismatch) };
    }
    let files = verify_closure(path.parent().ok_or(Error::LibraryIdentityMismatch)?, pins)?;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) { return Err(Error::LibraryIdentityMismatch); }
    wide.push(0);
    let module = Module(NonNull::new(unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(),
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) })
        .ok_or(Error::LibraryUnavailable)?);
    if module_path(module.0.as_ptr())? != path {
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
    *slot = Some(Loaded { path, pins: pins.to_owned(), api, _files: files });
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pins_reject_unreviewed_or_ambiguous_names() {
        let hash = "a".repeat(64);
        let valid = format!("{DLL_NAME}={hash};cublas64_12.dll={hash}");
        assert_eq!(parse_pins(&valid).unwrap().len(), 2);
        for invalid in [
            String::new(), format!("other.dll={hash}"),
            format!("{DLL_NAME}={hash};{DLL_NAME}={hash}"),
            format!("{DLL_NAME}={hash};../other.dll={hash}"),
            format!("{DLL_NAME}=invalid"),
            format!("{DLL_NAME}={hash};CUBLAS64_12.dll={hash}"),
        ] {
            assert!(parse_pins(&invalid).is_err());
        }
    }

    #[test]
    fn exact_private_file_set_and_hash_are_required_before_load() {
        let root = std::env::temp_dir().join(format!("translateit-dll-pins-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        // These are inert bytes, never passed to LoadLibraryExW.
        let name = DLL_NAME;
        let bytes = b"fixture-only, not a DLL";
        let hash = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let pins = format!("{name}={hash}");
        let path = root.join(name);
        std::fs::write(&path, bytes).unwrap();
        let locked = verify_closure(&root, &pins).expect("pinned inert file");
        assert!(std::fs::write(&path, b"changed").is_err());
        drop(locked);
        std::fs::write(&path, b"changed").unwrap();
        assert!(matches!(verify_closure(&root, &pins), Err(Error::LibraryIdentityMismatch)));
        std::fs::write(&path, bytes).unwrap();
        std::fs::write(root.join("extra.dll"), b"unreviewed").unwrap();
        assert!(matches!(verify_closure(&root, &pins), Err(Error::LibraryIdentityMismatch)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_relative_and_wrong_library_names_before_loading() {
        assert_eq!(controlled_path(Path::new(DLL_NAME)), Err(Error::LibraryIdentityMismatch));
        assert_eq!(controlled_path(Path::new(r"C:\TranslateIT\other.dll")),
            Err(Error::LibraryIdentityMismatch));
    }
}
