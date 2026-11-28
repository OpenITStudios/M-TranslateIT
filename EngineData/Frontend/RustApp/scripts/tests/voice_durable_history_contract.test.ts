import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (p: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + p), "utf8");

test("worker dataset remains unchanged while all training takes have durable lineage", () => {
  const storage = rust("voice_lab/storage.rs");
  const dataset = rust("voice_lab/storage/dataset_history.rs");
  const archive = rust("voice_lab/storage/recording_archive.rs");
  const freeze = rust("voice_lab/storage/archive_dataset.rs");
  assert.match(storage, /archive_dataset::prepare_archived_dataset\(storage, manifest, &archive_ids, false\)\?/);
  assert.match(storage, /archive_guided_take\(/);
  assert.match(freeze, /dataset_history::preserve_frozen_dataset_with_archives\(storage, manifest, archive_ids, from_saved_archive\)\?/);
  assert.doesNotMatch(freeze.split("#[cfg(test)]")[0], /join\("NativeRate"\)|source\.native_audio/);
  assert.match(dataset, /verify_saved_take_binding\(/);
  assert.match(dataset, /persist_manifest\(/);
  assert.match(dataset, /frozen_dataset_snapshot_id/);
  assert.match(archive, /receipt\.line_id != line_id \|\| receipt\.exact_text != exact_text/);
  assert.match(freeze, /write_json\(&storage\.build_dataset_dir\.join\("dataset.json"\), manifest\)\?/);
});

test("actor replacement preserves active, stored and approved candidate package", () => {
  const storage = rust("voice_lab/storage.rs");
  const history = rust("voice_lab/storage/actor_history.rs");
  const promote = storage.indexOf("pub(super) fn promote_voice_actor_candidate_at");
  const stage = storage.indexOf("let staging =", promote);
  assert.ok(promote !== -1 && stage > promote);
  const prepromotion = storage.slice(promote, stage);
  assert.match(prepromotion, /preserve_actor_version\(storage, &storage\.approved_actor_dir\)/);
  assert.match(prepromotion, /preserve_actor_version\(storage, &storage\.candidate_actor_dir\)/);
  assert.match(prepromotion, /preserve_training_lineage/);
  assert.match(storage, /preserve_actor_version\(paths, current\)/);
  assert.match(storage, /preserve_actor_version\(paths, &paths\.approved_actor_dir\)/);
  assert.match(history, /hash_regular_file/);
  assert.match(history, /create_dir\(&pending\)/);
  assert.match(history, /verify_version\(&saved/);
  assert.match(history, /different_approved_actors_remain_individually_recoverable/);
});

test("new durable history is independent of active engine and no restore is advertised yet", () => {
  const rootCargo = readFileSync(resolve(root, "src-tauri/Cargo.toml"), "utf8");
  const lock = readFileSync(resolve(root, "src-tauri/Cargo.lock"), "utf8");
  const versions = rust("voice_lab/storage/actor_history.rs");
  const data = rust("voice_lab/storage/dataset_history.rs");
  assert.match(rootCargo, /sha2 = "=0\.11\.1"/);
  assert.match(lock, /name = "sha2"\nversion = "0\.11\.1"/);
  assert.match(versions, /NOT an optimizer training checkpoint/);
  assert.match(versions, /No deletion, no activation/);
  assert.match(data, /Historical manifests/);
});

test("interrupted current-take Accept blocks new training before frozen dataset deletion", () => {
  const source = rust("voice_lab/storage.rs");
  const gate = source.indexOf("fn ensure_accepted_take_sources_safe(");
  const entry = source.indexOf("pub(super) fn prepare_guided_dataset_at(");
  const checked = source.indexOf("let verified = ensure_accepted_take_sources_safe(storage, manifest)?;", entry);
  const freeze = source.indexOf("archive_dataset::prepare_archived_dataset(storage, manifest, &archive_ids, false)?;", checked);
  const archive = rust("voice_lab/storage/archive_dataset.rs");
  const deletion = archive.indexOf("fs::remove_dir_all(&storage.build_dataset_dir)");
  const copy = archive.indexOf("write_new_synced(&target, &source.canonical_audio)");
  assert.ok(gate >= 0 && entry > gate && checked > entry && freeze > checked && deletion >= 0 && copy > deletion,
    "read-only Take-source admission must happen before destructive dataset replacement");
  const backupGuard = source.indexOf("fn ensure_accepted_take_backup_absent(");
  assert.ok(backupGuard >= 0 && backupGuard < gate);
  assert.match(source.slice(backupGuard, gate), /fs::symlink_metadata\(path\)[\s\S]*?accepted_take_interrupted_save_recovery_required/);
  assert.match(source.slice(gate, entry), /ensure_accepted_take_backup_absent\(&canonical\.with_extension\("wav\.previous"\)\)\?/);
  assert.match(source.slice(gate, entry), /ensure_accepted_take_backup_absent\(&native\.with_extension\("wav\.previous"\)\)\?/);
  assert.match(source.slice(gate, entry), /recording_archive::verified_archive_id_for_pair/);
  assert.match(source.slice(gate, entry), /voice_lab:accepted_native_pair_archive_unverified/);
  assert.match(source.slice(checked, freeze), /Some\(id\) => id/);
  assert.match(source.slice(checked, freeze), /None => archive_guided_take/);
  assert.match(rust("voice_lab.rs"), /fn native_reference_freeze_retains_sidecar_in_immutable_archive_only/);
  assert.match(source, /mod interrupted_accepted_dataset_admission_tests/);
  assert.match(source, /canonical_only_legacy_take_keeps_its_existing_source_contract/);
  assert.match(source, /either_interrupted_previous_backup_blocks_before_dataset_mutation/);
});
