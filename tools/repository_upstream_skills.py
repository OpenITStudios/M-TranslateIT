"""Static integrity and activation-boundary validation for pinned external Skill sources.

This module reads data only. It does not execute scripts, install plugins, start
subagents, dispatch CI, select a product owner, or claim model-level routing.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_SOURCE_COMMIT = "4ae2317ddb22b046e26455e4f9c9b3aaaf62212d"
EXPECTED_SOURCE_FILES = 119
EXPECTED_SKILLS = 39
VENDOR = Path(".agents/vendor/mattpocock")
SNAPSHOT = VENDOR / "snapshot.json"
CATALOG = Path(".agents/methods/catalog.json")
METHOD_CASES = Path(".agents/evals/method-overlay-cases.json")
VALID_ACTIVATIONS = {"conditional", "explicit-only", "reference-only"}
PROTECTED = {"ask-matt", "pr", "setup-matt-pocock-skills",
             "git-guardrails-claude-code", "migrate-to-shoehorn",
             "setup-pre-commit"}


def git_blob_sha(payload: bytes) -> str:
    header = f"blob {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


def _valid_source_path(path: object) -> bool:
    if not isinstance(path, str) or not (
            path.startswith("skills/") or path in {"LICENSE", "README.md", "SCOPE.md",
                    ".agents/invocation.md", ".claude-plugin/plugin.json"}
        ):
        return False
    allowed_roots = {"skills", "LICENSE", "README.md", "SCOPE.md",
                     ".agents", ".claude-plugin"}
    parts = path.split("/")
    return (parts[0] in allowed_roots and all(p not in {"", ".", ".."} for p in parts)
            and "\\" not in path and "\x00" not in path and not path.startswith("/"))


def verify_upstream(root: Path = ROOT) -> list[str]:
    """Check exact pinned bytes and inventory; never import the upstream code."""
    errors: list[str] = []
    base = root / VENDOR
    try:
        manifest = json.loads((root / SNAPSHOT).read_text(encoding="utf-8"))
        methods = json.loads((root / CATALOG).read_text(encoding="utf-8"))
        corpus = json.loads((root / METHOD_CASES).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return [f"missing or malformed external Skill metadata: {exc}"]
    if manifest.get("schemaVersion") != 1 or methods.get("schemaVersion") != 1:
        errors.append("unsupported pinned snapshot/catalog schema")
    if manifest.get("repository") != "mattpocock/skills" or manifest.get("license") != "MIT":
        errors.append("unrecognized upstream identity/license")
    if (not re.fullmatch(r"[0-9a-f]{40}", str(manifest.get("commit", "")))
            or manifest.get("commit") != EXPECTED_SOURCE_COMMIT):
        errors.append("upstream commit does not match this reviewed pinned version")
    sources = manifest.get("files", [])
    if not isinstance(sources, list):
        return errors + ["snapshot files are not a list"]
    indexed: dict[str, dict] = {}
    for entry in sources:
        if not isinstance(entry, dict) or not _valid_source_path(entry.get("path")):
            errors.append(f"unsafe snapshot source entry: {entry!r}")
            continue
        rel = entry["path"]
        if rel in indexed:
            errors.append("duplicate upstream source path: " + rel)
            continue
        indexed[rel] = entry
        path = base / rel
        if path.is_symlink() or not path.is_file():
            errors.append("missing or linked upstream source: " + rel)
            continue
        data = path.read_bytes()
        if len(data) != entry.get("size") or git_blob_sha(data) != entry.get("sha"):
            errors.append("upstream content drift: " + rel)
    expected = set(indexed)
    actual: set[str] = set()
    if base.exists():
        for path in base.rglob("*"):
            if path.is_symlink():
                errors.append("linked upstream entry: " + path.relative_to(base).as_posix())
            if path.is_file() and path.relative_to(base).as_posix() != "snapshot.json":
                actual.add(path.relative_to(base).as_posix())
    if actual != expected:
        errors.append(f"upstream inventory drift: missing={sorted(expected - actual)} extras={sorted(actual - expected)}")
    if manifest.get("sourceFileCount") != len(expected) or len(expected) != EXPECTED_SOURCE_FILES:
        errors.append("source file count does not match reviewed snapshot")
    license_entry = indexed.get("LICENSE")
    if license_entry is None or manifest.get("licensePath") != "LICENSE":
        errors.append("missing pinned upstream MIT attribution")
    else:
        try:
            if "MIT License" not in (base / "LICENSE").read_text(encoding="utf-8"):
                errors.append("upstream MIT license header missing")
        except OSError:
            errors.append("license file unreadable")

    definition_paths = {p for p in indexed if p.startswith("skills/") and p.endswith("/SKILL.md")}
    if (len(definition_paths) != manifest.get("upstreamSkillCount")
            or len(definition_paths) != EXPECTED_SKILLS):
        errors.append("upstream SKILL.md count mismatch")
    records = methods.get("methods", [])
    if not isinstance(records, list):
        return errors + ["method catalog is not a list"]
    recorded: set[str] = set()
    ids: set[str] = set()
    for item in records:
        if not isinstance(item, dict):
            errors.append("malformed method record")
            continue
        name, bucket, path = item.get("id"), item.get("bucket"), item.get("sourcePath")
        activation = item.get("activation")
        if not isinstance(name, str) or not isinstance(bucket, str) or not isinstance(path, str):
            errors.append("malformed method identity")
            continue
        if path in recorded or name in ids:
            errors.append("duplicate method identity/path: " + name)
        recorded.add(path)
        ids.add(name)
        if path != f"skills/{bucket}/{name}/SKILL.md" or path not in definition_paths:
            errors.append("method does not match upstream source: " + name)
            continue
        if activation not in VALID_ACTIVATIONS or item.get("sourceSha") != indexed[path]["sha"]:
            errors.append("unapproved method activation/source SHA: " + name)
        if bucket == "in-progress" and activation != "reference-only":
            errors.append("beta method is not quarantined: " + name)
        if bucket == "misc" and activation == "conditional":
            errors.append("misc method activated implicitly: " + name)
        if name in PROTECTED and activation != "reference-only":
            errors.append("upstream router/setup/automation activated: " + name)
        content = (base / path).read_text(encoding="utf-8") if (base / path).is_file() else ""
        if re.search(r"(?m)^disable-model-invocation:\s*true\s*$", content):
            if activation == "conditional":
                errors.append("human-invoked upstream Skill made implicit: " + name)
    if recorded != definition_paths or len(records) != len(definition_paths):
        errors.append("catalog must account for every upstream Skill once")
    if methods.get("source") != "../vendor/mattpocock/snapshot.json":
        errors.append("method catalog source pointer is not pinned")

    cases = corpus.get("cases", [])
    kinds = set()
    case_ids: set[str] = set()
    if not isinstance(cases, list):
        return errors + ["method overlay cases must be a list"]
    mode_by_id = {x.get("id"): x.get("activation") for x in records if isinstance(x, dict)}
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            errors.append("invalid overlay case")
            continue
        cid, kind, chosen = case["id"], case.get("kind"), case.get("expectedMethod")
        if cid in case_ids:
            errors.append("duplicate method eval id: " + cid)
        case_ids.add(cid)
        kinds.add(kind)
        if not isinstance(case.get("prompt"), str) or not case["prompt"].strip():
            errors.append("missing overlay case prompt: " + cid)
        if chosen is not None and mode_by_id.get(chosen) not in {"conditional", "explicit-only"}:
            errors.append("case expects unavailable/disabled method: " + cid)
        if kind == "no-skill" and chosen is not None:
            errors.append("no-skill case activated method: " + cid)
    if not {"positive", "collision", "pressure", "no-skill"} <= kinds:
        errors.append("missing positive/collision/pressure/no-skill overlay coverage")
    return sorted(set(errors))


def score_method_runs(runs: list[dict], root: Path = ROOT) -> dict:
    """Score *observed* external agent selections; never runs an agent."""
    corpus = json.loads((root / METHOD_CASES).read_text(encoding="utf-8"))
    expected = {case["id"]: case for case in corpus["cases"]}
    selected: dict[str, object] = {}
    duplicates: list[str] = []
    for run in runs:
        key = run.get("caseId") if isinstance(run, dict) else None
        if not isinstance(key, str) or key in selected:
            duplicates.append(str(key))
            continue
        selected[key] = run.get("selectedMethod")
    mismatch = []
    by_kind: dict[str, dict[str, int]] = {}
    for cid, case in expected.items():
        kind = case["kind"]
        tally = by_kind.setdefault(kind, {"total": 0, "correct": 0})
        tally["total"] += 1
        ok = cid in selected and selected[cid] == case["expectedMethod"]
        if ok:
            tally["correct"] += 1
        else:
            mismatch.append({"caseId": cid, "expected": case["expectedMethod"],
                             "actual": selected.get(cid, "MISSING")})
    extras = sorted(set(selected) - set(expected))
    return {"total": len(expected), "correct": len(expected) - len(mismatch),
            "byKind": by_kind, "mismatches": mismatch,
            "duplicateRuns": duplicates, "unknownRuns": extras,
            "evidence": "USER_SUPPLIED_OBSERVED_RUNS_NOT_VERIFIED_BY_SCORER"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="read-only static audit")
    parser.add_argument("--score", type=Path, metavar="RUNS_JSON",
                        help="score supplied observed agent method selections")
    args = parser.parse_args()
    errors = verify_upstream()
    if errors:
        for issue in errors:
            print("FAIL: " + issue)
        return 1
    print("PASS: pinned external Skill snapshot, all 39 methods and activation boundaries")
    print("SOURCE ONLY: no model/runtime/Windows execution performed")
    if args.score:
        try:
            data = json.loads(args.score.read_text(encoding="utf-8"))
            if not isinstance(data, dict) or not isinstance(data.get("runs"), list):
                raise ValueError("expected {runs: [ {caseId, selectedMethod}, ... ]}")
            scores = score_method_runs(data["runs"])
        except (OSError, ValueError, TypeError) as exc:
            print("FAIL: invalid observed-run receipt: " + str(exc))
            return 1
        print(json.dumps(scores, indent=2))
        return 0 if (scores["correct"] == scores["total"]
                     and not scores["duplicateRuns"] and not scores["unknownRuns"]) else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
