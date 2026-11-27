use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=TRANSLATEIT_NATIVE_LIB_DIR");
    assert!(matches!(env::var("TARGET").as_deref(), Ok("x86_64-pc-windows-msvc")),
        "candidate is Windows x64 MSVC only");
    let path = PathBuf::from(env::var_os("TRANSLATEIT_NATIVE_LIB_DIR")
        .expect("provide the exact candidate DLL/import-library directory"));
    assert!(path.is_absolute() && path.join("translateit_native_cuda.lib").is_file(),
        "candidate import library missing");
    println!("cargo:rustc-link-search=native={}", path.display());
    println!("cargo:rustc-link-lib=dylib=translateit_native_cuda");
    println!("cargo:rerun-if-changed={}", path.join("translateit_native_cuda.lib").display());
}
