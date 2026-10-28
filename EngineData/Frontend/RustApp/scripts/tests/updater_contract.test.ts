import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(testDir, "../..");

const read = (path: string) => readFileSync(join(appRoot, path), "utf8");

test("developer startup supplies updater configuration without enabling downloads", () => {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  assert.deepEqual(config.plugins.updater, { pubkey: "", endpoints: [] });
});

test("updater remains one-shot and begins from the app root", () => {
  const api = read("src/app/update/appUpdateApi.ts");
  const app = read("src/App.svelte");
  const action = read("src/components/runtime/UpdateAction.svelte");

  assert.match(api, /let startupCheckPromise: Promise<AppUpdateCheck \| null> \| null = null;/);
  assert.match(api, /if \(!startupCheckPromise\)/);
  assert.match(api, /startupCheckPromise = runCommand<AppUpdateCheck>\("check_app_update_once"\)/);
  assert.match(api, /return startupCheckPromise;/);
  assert.match(app, /appUpdateApi\.checkAtStartupOnce\(\)/);
  assert.match(action, /checkAtStartupOnce\(\)/);

  for (const source of [api, action]) {
    assert.doesNotMatch(source, /setInterval\s*\(/);
    assert.doesNotMatch(source, /setTimeout\s*\(/);
    assert.doesNotMatch(source, /requestAnimationFrame\s*\(/);
  }
});

test("native updater is signed, GitHub-release based, and activity-safe", () => {
  const source = read("src-tauri/src/commands/app_update.rs");
  const header = read("src/components/layout/AppHeader.svelte");
  const action = read("src/components/runtime/UpdateAction.svelte");

  assert.match(source, /option_env!\("TRANSLATEIT_UPDATER_PUBLIC_KEY"\)/);
  assert.match(source, /releases\/latest\/download\/latest\.json/);
  assert.match(source, /voice_lab_recording_blocks_app_exit/);
  assert.match(source, /voice_lab_build_blocks_meeting\(\)/);
  assert.match(source, /runtime\.has_active_session/);
  assert.match(header, /audioLocked=\{snapshot\.resources\.audio_locked\}/);
  assert.match(header, /audioOwnerKind=\{snapshot\.resources\.owner_kind\}/);
  assert.match(header, /voiceBuildActive=\{snapshot\.voiceBuildActive\}/);
  assert.match(header, /<UpdateAction/);
  assert.match(header, /audioLocked=\{snapshot\.resources\.audio_locked\}/);
  assert.match(header, /audioOwnerKind=\{snapshot\.resources\.owner_kind\}/);
  assert.match(header, /voiceBuildActive=\{snapshot\.voiceBuildActive\}/);
  assert.match(header, /\{myVoiceRecording\}/);
  assert.match(action, /audioLocked/);
  assert.match(action, /audioOwnerKind/);
  assert.match(action, /voiceBuildActive/);
  assert.match(action, /myVoiceRecording/);
  assert.match(action, /Stop Translation before updating TranslateIT/);
  assert.match(action, /Stop Mic Test before updating TranslateIT/);
  assert.match(action, /Stop the current My Voice recording before updating TranslateIT/);
  assert.match(action, /Wait for My Voice creation to finish before updating TranslateIT/);
  assert.ok((source.match(/update_blocker\(\)/g) ?? []).length >= 2);
  assert.ok(source.lastIndexOf("update_blocker()") < source.indexOf("download_and_install"));
  assert.match(source, /download_and_install/);

  assert.doesNotMatch(source, /loop\s*\{/);
  assert.doesNotMatch(source, /std::thread::spawn/);
});

test("release build fails closed without updater signing material", () => {
  const release = read("scripts/build_release.ps1");
  const releaseConfig = JSON.parse(read("src-tauri/tauri.release.conf.json"));

  assert.match(release, /TRANSLATEIT_UPDATER_PUBLIC_KEY/);
  assert.match(release, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(release, /TAURI_SIGNING_PRIVATE_KEY_PATH/);
  assert.equal(releaseConfig.bundle?.createUpdaterArtifacts, true);
});


test("external runtime payload is preserved only when installed runtime is compatible", () => {
  const hook = read("src-tauri/windows/r3_payload_hooks.template.nsh");
  const helper = read("src-tauri/windows/r3_payload_installer.ps1");

  assert.match(hook, /GetOptions.*"\/UPDATE"/);
  assert.match(hook, /-Mode VerifyInstalled/);
  assert.match(hook, /preserving the verified installed runtime payload/);
  assert.match(helper, /ValidateSet\('Verify','Install','VerifyInstalled'\)/);
  assert.match(helper, /function Verify-InstalledRuntime/);
  assert.match(helper, /Assert-InstalledRuntimeManifest/);
  assert.match(helper, /translation_revision/);
  assert.match(helper, /asr_revision/);
  assert.match(helper, /voice_revision/);
});


test("updater reserves exclusive native admission through installer handoff", () => {
  const source = read("src-tauri/src/commands/app_update.rs");
  const state = read("src-tauri/src/engine/runtime_state.rs");
  assert.match(source, /reserve_app_update_install\(\|\| update_blocker\(\)\)/);
  assert.match(source, /update_lease\.keep_reserved_until_exit\(\)/);
  assert.ok(source.indexOf("reserve_app_update_install") < source.indexOf("download_and_install"));
  assert.match(source, /meeting_stop_cleanup_pending\(\)/);
  assert.match(source, /voice_lab_build_blocks_meeting\(\)/);
  assert.match(source, /voice_backup_active\(\)/);
  assert.match(source, /runtime\.snapshot\.is_some\(\)/);
  assert.match(source, /runtime\.blocker != "runtime_session:no_active_session"/);

  assert.match(state, /struct UpdateAdmissionState/);
  assert.match(state, /active_operations: usize/);
  assert.match(state, /if state\.installing/);
  assert.match(state, /if state\.active_operations != 0/);
  assert.match(state, /if let Some\(reason\) = blocker\(\)/);
  assert.match(state, /state\.installing = true/);
  assert.match(state, /impl Drop for AppUpdateInstallPermit/);
  assert.match(state, /if self\.release_on_drop/);
  assert.match(state, /fn begin_runtime_session\([\s\S]*?admit_app_runtime_operation\(\)/);
});

test("Meeting and My Voice native mutation owners use the same updater admission", () => {
  const runtime = read("src-tauri/src/commands/runtime.rs");
  const voice = read("src-tauri/src/commands/voice_lab.rs");
  const voiceBuild = read("src-tauri/src/commands/voice_lab_build.rs");
  const recording = read("src-tauri/src/commands/voice_lab_recording.rs");

  assert.match(runtime, /fn admit_meeting_start\(\)[\s\S]*?admit_app_runtime_operation\(\)/);
  assert.match(runtime, /fn start_meeting_translation\(\)[\s\S]*?admit_meeting_start\(\)/);
  assert.match(runtime, /fn start_meeting_original\(\)[\s\S]*?admit_meeting_start\(\)/);
  assert.match(voice, /fn begin_voice_lab_build\(\)[\s\S]*?admit_app_runtime_operation\(\)/);
  assert.match(voiceBuild, /fn voice_selection_guard\(\)[\s\S]*?admit_app_runtime_operation\(\)/);
  assert.match(voiceBuild, /if voice_backup_active\(\)/);
  assert.match(recording, /fn admit_recording_file_change\([\s\S]*?admit_app_runtime_operation\(\)/);
  for (const operation of ["recover_voice_lab_guided_take", "retry_voice_lab_guided_take", "accept_voice_lab_guided_take"]) {
    const start = recording.indexOf("pub fn " + operation);
    assert.ok(start >= 0, operation + " must stay native");
    assert.ok(recording.slice(start, start + 370).includes("admit_recording_file_change()"),
      operation + " must hold update admission for its file operation");
  }
  assert.match(recording, /fn protect_voice_lab_legacy_recordings\(\)[\s\S]*?admit_app_runtime_operation\(\)/);
});

test("failed update attempts surface an actionable status", () => {
  const action = read("src/components/runtime/UpdateAction.svelte");
  assert.ok(action.includes("Couldn't confirm the update result. Check your app version before trying again."));
  assert.match(action, /\} catch \{[\s\S]*?\} finally \{\s*busy = false;/);
});
