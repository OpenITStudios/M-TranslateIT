import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (path: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + path), "utf8");

test("historical checkpoint preservation stays bound to exact legacy source and dataset", () => {
  const recovery = rust("voice_lab/storage/training_recovery.rs");
  assert.match(recovery, /VOICE_ACTOR_ENGINE_REVISION/);
  assert.match(recovery, /frozen_dataset_snapshot_id\(storage\)/);
  assert.match(recovery, /checkpoint_preserved_not_verified_for_resume/);
  assert.match(recovery, /sovits-G/);
  assert.match(recovery, /sovits-D/);
  assert.match(recovery, /gpt-config\.yaml/);
  assert.match(recovery, /sovits-config\.json/);
  assert.match(recovery, /verify_source_pair\(&input\.source, &parts\)/);
});

test("native creation preserves interrupted legacy work before changing dataset or clearing cache", () => {
  const build = rust("voice_lab_build/native_build.rs");
  const recovery = rust("voice_lab/storage/training_recovery.rs");
  const capture = build.indexOf("preserve_training_checkpoints(paths)?");
  const legacyClear = build.indexOf('remove_dir_if_present(&work_dir(paths), "work")?', capture);
  const prepare = build.indexOf("prepare_guided_dataset_from_archives(", legacyClear);
  const clear = build.indexOf("clear_previous_build_workspace(paths)?", prepare);
  assert.ok(capture >= 0 && legacyClear > capture && prepare > legacyClear && clear > prepare);
  const owner = rust("voice_lab_build.rs");
  const cleanup = owner.slice(owner.indexOf("fn clear_previous_build_workspace("), owner.indexOf("fn a3_wav_duration_ms("));
  assert.doesNotMatch(cleanup, /remove_dir_if_present\(&work_dir\(paths\)/);
  assert.match(recovery, /create_dir\(&pending\)/);
  assert.match(recovery, /verify_package\(&pending/);
  assert.match(recovery, /fs::rename\(&pending, &complete\)/);
  assert.match(recovery, /recovery_checkpoint_changed_during_copy/);
  assert.match(recovery, /incomplete_training_checkpoint_set_is_preserved_for_investigation_not_resumed/);
  assert.doesNotMatch(recovery.split("#[cfg(test)]")[0], /remove_file\(|remove_dir_all\(/);
});

test("backup preserves recovery objects but cannot resume retired training", () => {
  const backup = rust("voice_lab/storage/backup.rs");
  const registry = rust("registry.rs");
  const recording = rust("voice_lab_recording.rs");
  const ui = readFileSync(resolve(root, "src/components/my-voice/VoiceBackupRestore.svelte"), "utf8");
  assert.match(backup, /"TrainingRecovery"/);
  assert.match(backup, /valid_recovery_file/);
  assert.match(backup, /recovery_receipt_identity_invalid/);
  assert.match(backup, /recovery_dataset_missing/);
  assert.match(backup, /recovery_file_changed/);
  assert.match(registry, /get_voice_lab_training_recovery_status/);
  assert.match(recording, /pub fn get_voice_lab_training_recovery_status/);
  assert.match(ui, /preserved_checkpoint_candidates/);
  assert.doesNotMatch(ui, /onclick=.*resumeVoiceTraining/i);
});

test("native Create and Improve reject unsupported platforms before work", () => {
  const build = rust("voice_lab_build.rs");
  const start = build.indexOf("fn start_voice_lab_build_selected(");
  const gate = build.indexOf("if !cfg!(windows)", start);
  const first = build.indexOf("if voice_backup_active()", start);
  const dispatch = build.indexOf("native_build::start(current, selected_archives)", start);
  assert.ok(start >= 0 && gate > start && first > gate && dispatch > first);
  assert.doesNotMatch(build, /command\.spawn\(\)|OwnedBuildJob/);
});
