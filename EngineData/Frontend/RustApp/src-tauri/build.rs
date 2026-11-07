use std::{env, process::Command};

// Build identity is diagnostic only, never a signed-release attestation.
fn clean_pinned_source_sha() -> Option<String> {
    let expected = env::var("TRANSLATEIT_EXPECTED_SHA").ok()?;
    if expected.len() != 40 ||
        !expected.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")?;
    let run = |args: &[&str]| Command::new("git")
        .args(args).current_dir(&manifest_dir).output().ok()
        .filter(|result| result.status.success());
    let head = run(&["rev-parse", "HEAD"])?;
    let status = run(&["status", "--porcelain", "--untracked-files=no"])?;
    if String::from_utf8(head.stdout).ok()?.trim() != expected || !status.stdout.is_empty() {
        return None;
    }
    Some(expected)
}

fn main() {
    println!("cargo:rerun-if-env-changed=TRANSLATEIT_EXPECTED_SHA");
    if let Some(sha) = clean_pinned_source_sha() {
        println!("cargo:rustc-env=TRANSLATEIT_BUILD_SOURCE_SHA={sha}");
    }
    tauri_build::build();
}
