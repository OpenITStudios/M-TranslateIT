# TranslateIT Engineering Method Runbook

This is a conditional procedure companion to root `AGENTS.md`. Select **one method only when it changes the decision**. Any linked upstream SKILL is reference data; where instructions conflict, `AGENTS.md`, `GITHUB_RULES.md`, product law, permission preflight and current semantic owner win.

## Bug diagnosis: tight signal before broad search

Use [diagnosing-bugs](../vendor/mattpocock/skills/engineering/diagnosing-bugs/SKILL.md) only on actual broken behavior or measurable regression.

1. Pin exact `Local` commit, the user's observed symptom and expected outcome. Identify first observable wrong boundary across UI ↔ Rust ↔ worker ↔ Windows audio. Start from the owner's error/status codes or a failing end-to-end seam, not speculative full-repo searches.
2. Seek a **red-capable, symptom-specific, fast** test or fixture using the current available context. In `REMOTE_GITHUB`, write/review that regression contract statically and record `NOT_EXECUTED`; do not pretend the loop was actually run. Do not request incremental PC tests or enable CI.
3. With an observed failing signal, minimise the reproduction and rank falsifiable hypotheses by discriminating evidence. If no loop can be run, distinguish *source-backed likely owner* from *proven runtime cause*. Do not repeatedly trial changes without a measurable signal.
4. Probe one causal boundary/variable. Change smallest responsible implementation owner. Review direct callers and contracts, then verify at the available proof ceiling. Keep hard runtime/Windows evidence deferred until integrated acceptance.
5. Stop when the source bug and regression contract are complete, or write a bounded handoff with exact missing evidence and no fabricated PASS.

## Test-first feature or repair

Use [tdd](../vendor/mattpocock/skills/engineering/tdd/SKILL.md) with [implement](../vendor/mattpocock/skills/engineering/implement/SKILL.md) as *techniques*, not extra project skills.

- Name one existing public seam and an independent expected outcome; prefer the production consumer over a shallow test surrogate.
- Test → minimal production change → focused verification → next vertical slice. **Red/green requires actual execution**; a new test file is only source preparation in `REMOTE_GITHUB`.
- Avoid refactor-only churn, hypothetical tests, duplicate import contracts, toolchain installs or CI dispatch. Follow `tools/interop-contracts.json`, `tools/repository_impact.py`, and `tools/repository_verification.py` for affected proof routing.

## Code review, specification and architecture

- [code-review](../vendor/mattpocock/skills/engineering/code-review/SKILL.md): pin the comparison ref and changed files. Produce separate **Standards** (repo rules + security + conventions) and **Spec** (the stated requirement and acceptance) findings. If no separate subagent is available/approved, perform sequential independent passes, not imagined parallel reviews. Direct `Local` commits do not need a PR.
- [to-spec](../vendor/mattpocock/skills/engineering/to-spec/SKILL.md) / [domain-modeling](../vendor/mattpocock/skills/engineering/domain-modeling/SKILL.md): resolve only material missing decisions through existing `development-brief`; do not rename `CONTEXT.md`, create conflicting ADR/doc ledgers, or migrate an established issue tracker.
- [codebase-design](../vendor/mattpocock/skills/engineering/codebase-design/SKILL.md) / [improve-codebase-architecture](../vendor/mattpocock/skills/engineering/improve-codebase-architecture/SKILL.md): measure a real ownership/consumer/dependency smell, propose alternatives, keep the current single runtime and interop authority. Architecture work is not permission to rewrite product law.

## Agent-rule and Skill quality work

Use [writing-for-agents](../vendor/mattpocock/skills/productivity/writing-for-agents/SKILL.md) as a writing/refinement technique and the M-LazyDesigner pattern of **eval-driven skill engineering**: explain activation owner, positive trigger, negative trigger, collision, pressure, and no-skill case; inspect existing `.agents/evals` before changing an active Skill. Prefer frontmatter routing fixes and small local procedures over duplicating global law in each Skill.

`tools/repository_agent_evals.py --check` and `tools/repository_skill_admission.py --check` remain the canonical six-skill checks. `tools/repository_upstream_skills.py --check` protects this optional external snapshot and method inventory. All are static; actual model selection and real runtime behavior remain separately unverified until observed.

## External setup and non-promoted methods

`setup-matt-pocock-skills`, `ask-matt`, `pr`, shell hooks, external issue-tracker automation, `misc` and `in-progress` methods are **reference-only or explicit-only as recorded in the catalog**; they must not modify repo configuration, install tools, run scripts, request permissions or create branches/PRs automatically. An explicit request is still subject to original repository safety gates.

For a real handoff, report `observed`, `expected`, `reproduction/evidence`, `source_owner`, `target_owner`, `resume_stage`, `proof_ceiling`. A handoff is not automatic routing.
