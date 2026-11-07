# TranslateIT Tools

Repository-owned Python tools for **source-derived verification, routing, impact and model-quality contracts**. They are not a second application/runtime owner and do not create an automatic agent or CI loop.

- `verify_repository.py` — aggregate structural and governance source checks
- `repository_knowledge.py` — source-linked document Catalog/typed Graph/derived sections, scoped retrieval, path tracing and read-only Obsidian/Pele-compatible Markdown/Mermaid exports; no Graphify/Pele/Obsidian installation or runtime proof
- `repository_context.py` — bounded read-only context/permission projection and optional `--knowledge-id` or domain-scoped `--knowledge-query` witnessed router navigation; `--external-audit-target source|artifact|runtime|capture` routes scoped REA audit evidence without executing REA or claiming external runtime proof
- `repository_rea.py` — opt-in REA CLI doctor, actual MCP stdio initialize/tools-list probe, and authorized JavaScript/ASAR inspection with bounded stdout bytes, approved version and private Evidence digest. No install or automatic target execution; other providers remain upstream-owned.
- `repository_dependencies.py`, `repository_impact.py` — partial source dependency impact and affected proof planning
- `repository_verification.py` — explicit opt-in execution of existing selected commands only
- `repository_benchmark.py` — opt-in source-derived method-flow/context/permission conformance replay, not model or native proof
- `repository_permissions.py`, `repository_agent_evals.py`, `repository_skill_admission.py` — scoped preflight, evaluation and skill package admission
- `repository_contracts.py` and `interop-contracts.json` — bridge contract location, not duplicate Rust/TS/Python semantics
- `translation_quality/`, `asr_quality/`, `tts_quality/`, `quality_readiness/` — independent current quality contracts; baseline fixture presence is not real inference PASS
- `audio_validation/` — deterministic hashed WAV fixture generator, strict observation scoring, explicit pinned local-worker smoke and ignored canonical Rust VAD/PCM16 receipt runner; never cloud/CI inference or production capture injection
- `tests/` — regression tests for these existing tools

`tooling/windows-toolchain/` contains the separate optional developer launcher invoked by root `DEV.cmd`. Source/product ownership is in [Implementation Map](../docs/knowledge/source-ownership.md); repository policy in [GitHub Rules](../GITHUB_RULES.md). No scripts are assigned to the user's PC for incremental ChatGPT-owned development.

## Pinned external Skill methods

`python tools/repository_upstream_skills.py --check` verifies the complete pinned MIT-licensed Matt Pocock `.agents/vendor/mattpocock/` snapshot and its `.agents/methods/catalog.json`/evaluation-case classification without executing any vendored script, installing agents, dispatching CI or claiming model-level correctness. See [method overlay](../.agents/methods/README.md). The six canonical project Skills and their separate admission/routing checks remain unchanged.
