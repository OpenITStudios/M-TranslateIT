---
id: document.system.knowledge-architecture
class: DOCUMENT
domain: system
role: ARCHITECTURE
authority: CANONICAL
lifecycle: ACTIVE
---

# Repository Knowledge Navigation Architecture

This document defines how an agent explains documented architecture, not what the product does. [Authority](./authority-model.md) remains the single owner of semantic precedence; [source ownership](../knowledge/source-ownership.md) names actual implementation owners. The [documentation router](../README.md) remains the entry point.

## Deterministic navigation

```text
current task + explicit work mode
→ docs/README.md → one domain router → canonical document ID
→ authored Markdown links and exact path:line witnesses
→ directed, typed document graph
→ bounded section and graph retrieval
→ optional current source/import impact and exact verification evidence
→ evidence-limited decision → STOP
```

Document frontmatter owns stable document identity (a rename keeps the ID when meaning is unchanged). H2/H3 section addresses are derived from each document's headings; duplicate headings use ordinal suffixes. Sections have DERIVED authority and point back to their one physical parent with DERIVED_FROM. Authored router links yield ROUTES_TO; other explicit links yield RELATES_TO. Neither relationship implies a code call, runtime behavior or approval.

## Source witnesses and missing paths

The existing [repository knowledge tool](../../tools/README.md) extracts links from Markdown prose with path and line witnesses; fenced code, inline code, HTML comments, images and indented examples are excluded. Obsidian-style links must use unambiguous repository-root paths such as [[docs/system/authority-model.md|authority owner]]; fuzzy note titles and aliases are not promoted into relationships. The trace follows only those registered edges, defaults to directed traversal within four hops, excludes HISTORICAL sources, and returns NO_PATH or UNKNOWN_RESOURCE when evidence is missing. Reverse paths and history require explicit opt-in.

The source graph remains a distinct layer: [source dependency analysis](./zero-waste-execution.md) is implemented by tools/repository_dependencies.py and tools/repository_impact.py and covers only bounded explicit Rust/TypeScript/Python imports. A Markdown link cannot stand in for a module dependency, compiler result, Windows behavior or executed proof. Unknown dynamic paths remain unknown.

## Read-only query examples

```text
python tools/repository_knowledge.py --check
python tools/repository_knowledge.py --summary
python tools/repository_knowledge.py --id document.system.authority-model
python tools/repository_knowledge.py --trace document.docs.router document.system.authority-model
python tools/repository_knowledge.py --trace document.docs.router document.system.authority-model --format markdown
python tools/repository_knowledge.py --trace document.system.authority-model document.docs.router --both-directions --format mermaid
python tools/repository_knowledge.py --query "Meeting audio" --domain knowledge
```

`--id` gives typed incoming and outgoing relationships and derived sections. `--trace` reports a bounded route with source witnesses. JSON is the default; Mermaid is diagram source (compatible with Pele), while Markdown contains explicit repo-root Obsidian links plus the Mermaid code fence. These are projections in stdout, not installed integrations, file writes, source verification or an Obsidian vault. Do not claim the renderer or Graphify itself executed.

## External method provenance and limits

- [Obsidian](https://obsidian.md/) informs human-readable, linkable Markdown knowledge; its note-name guessing does not elect repository owners.
- [Pele](https://github.com/obsidianmd/pele) renders Mermaid to SVG in an authorized presentation environment; this repository only emits Mermaid source, with no renderer dependency.
- [Graphify](https://github.com/Graphify-Labs/graphify) informs provenance-backed graph navigation and query-first workflows; this tool does not install Graphify, assert AST/call-graph completeness or create a second persistent graph store.

## Quality gate and evolution

One canonical owner per concern, no saved graph cache, no new agent Skills, no automatic CI/hosted inference, no branch promotion, and no private UserData ingestion. Graph extraction and query behavior are covered by source-authored regression tests in tools/tests/test_repository_knowledge.py. Each result has DOCUMENT_NAVIGATION_ONLY_NOT_SOURCE_OR_RUNTIME_PROOF. When a document is moved, update routers and real links as a bounded migration, then verify reachability; Git history retains superseded rationale.
