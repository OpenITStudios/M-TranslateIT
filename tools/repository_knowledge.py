"""Read-only, source-derived document Catalog → Graph → bounded Retrieval.

Frontmatter owns stable document IDs. Real Markdown links own edges. This module
never persists a second knowledge store or promotes retrieval ranking into proof.
"""
from __future__ import annotations

import argparse
from collections import Counter, deque
import json
import posixpath
import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
META = re.compile(r"\A---\r?\n(.*?)\r?\n---\r?\n", re.DOTALL)
LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
WIKI_LINK = re.compile(r"\[\[([^\]\n]+)\]\]")
HEAD = re.compile(r"(?m)^#\s+(.+)$")
SECTION = re.compile(r"(?m)^(#{2,3})\s+(.+)$")
WORDS = re.compile(r"[a-z0-9]+", re.I)
FIELDS = {"id", "class", "domain", "role", "authority", "lifecycle"}
ROLES = {"ROUTER", "WORKFLOW", "CONTRACT", "DOMAIN", "ARCHITECTURE", "GUIDE", "REFERENCE"}
AUTHORITIES = {"CANONICAL", "REFERENCE", "HISTORICAL", "DERIVED"}
ROUTERS = (
    "docs/README.md",
    "docs/foundation/README.md",
    "docs/knowledge/README.md",
    "docs/system/README.md",
    "docs/knowledge/decisions/README.md",
    "docs/knowledge/operations/README.md",
    "docs/knowledge/skills/README.md",
)


def read_metadata(content: str, path: str) -> dict[str, str]:
    match = META.match(content)
    if not match:
        raise ValueError(f"{path}: missing document metadata")
    fields: dict[str, str] = {}
    for line in match.group(1).splitlines():
        if ":" not in line:
            raise ValueError(f"{path}: malformed metadata line")
        key, value = line.split(":", 1)
        if key in fields or key not in FIELDS or not value.strip():
            raise ValueError(f"{path}: duplicate/unknown/empty metadata {key!r}")
        fields[key] = value.strip()
    if set(fields) != FIELDS:
        raise ValueError(f"{path}: metadata must contain exactly {sorted(FIELDS)}")
    if not re.fullmatch(r"document\.[a-z0-9-]+(?:\.[a-z0-9-]+)*", fields["id"]):
        raise ValueError(f"{path}: invalid stable document ID")
    if fields["class"] != "DOCUMENT" or fields["role"] not in ROLES:
        raise ValueError(f"{path}: invalid document classification")
    if fields["domain"] not in {"docs", "knowledge", "foundation", "system"}:
        raise ValueError(f"{path}: unsupported document domain")
    if fields["authority"] not in AUTHORITIES or fields["lifecycle"] not in {"ACTIVE", "RETIRED", "DRAFT"}:
        raise ValueError(f"{path}: invalid authority/lifecycle")
    if fields["lifecycle"] == "DRAFT" and fields["authority"] != "REFERENCE":
        raise ValueError(f"{path}: DRAFT must remain REFERENCE, never current authority")
    if fields["role"] == "ROUTER" and (fields["authority"] != "CANONICAL" or fields["lifecycle"] != "ACTIVE"):
        raise ValueError(f"{path}: documentation router must be active and canonical")
    return fields


def document_target(source: str, link: str) -> str | None:
    raw = unquote(link.strip().strip("<>")).split("#", 1)[0].split("?", 1)[0]
    if not raw or "://" in raw or raw.startswith(("mailto:", "/", "#")) or "\\" in raw:
        return None
    if not raw.lower().endswith(".md"):
        return None
    dest = posixpath.normpath(posixpath.join(posixpath.dirname(source), raw))
    return dest if dest.startswith("docs/") else None



# Navigation is derived only from authored prose, never from code examples.
# The same masked text supplies both Markdown witnesses and section headings.
def _prose_text(content: str) -> str:
    output: list[str] = []
    fence: tuple[str, int] | None = None
    comment = False
    fence_pattern = re.compile(r"^ {0,3}([" + chr(96) + r"~])\1{2,}")
    inline_code = re.compile("(" + re.escape(chr(96)) + r"+)(.*?)\1")
    for line in content.splitlines(keepends=True):
        raw = line.rstrip("\r\n")
        suffix = line[len(raw):]
        opening = fence_pattern.match(raw)
        if opening:
            width = len(opening.group(0).strip())
            if fence is None:
                fence = (opening.group(1), width)
            elif opening.group(1) == fence[0] and width >= fence[1]:
                fence = None
            output.append(" " * len(raw) + suffix)
            continue
        if fence or raw.startswith(("    ", "\t")):
            output.append(" " * len(raw) + suffix)
            continue
        chars = list(raw)
        index = 0
        while index < len(raw):
            if comment:
                closing = raw.find("-->", index)
                end = len(raw) if closing < 0 else closing + 3
                chars[index:end] = [" "] * (end - index)
                comment = closing < 0
                index = end
                continue
            opening_comment = raw.find("<!--", index)
            if opening_comment < 0:
                break
            closing = raw.find("-->", opening_comment + 4)
            end = len(raw) if closing < 0 else closing + 3
            chars[opening_comment:end] = [" "] * (end - opening_comment)
            comment = closing < 0
            index = end
        for span in inline_code.finditer("".join(chars)):
            chars[span.start():span.end()] = [" "] * (span.end() - span.start())
        output.append("".join(chars) + suffix)
    return "".join(output)


def _escaped_at(line: str, position: int) -> bool:
    return (position - len(line[:position].rstrip("\\"))) % 2 == 1


def authored_links(content: str) -> list[dict]:
    """Preserve literal path/line witnesses without claiming runtime semantics."""
    findings: list[dict] = []
    for lineno, line in enumerate(_prose_text(content).splitlines(), 1):
        for match in LINK.finditer(line):
            if (match.start() and line[match.start() - 1] == "!") or _escaped_at(line, match.start()):
                continue
            findings.append({"line": lineno, "syntax": "MARKDOWN", "target": match.group(1)})
        for match in WIKI_LINK.finditer(line):
            if (match.start() and line[match.start() - 1] == "!") or _escaped_at(line, match.start()):
                continue
            findings.append({"line": lineno, "syntax": "OBSIDIAN_WIKILINK", "target": match.group(1)})
    return findings


def wiki_target(raw: str) -> str | None:
    # Explicit repo-root paths only. Obsidian note aliases are ambiguous.
    value = unquote(raw.split("|", 1)[0].split("#", 1)[0].strip())
    if not value.startswith("docs/") or not value.endswith(".md") or "\\" in value:
        return None
    return value if posixpath.normpath(value) == value else None


def catalog(root: Path = ROOT) -> tuple[list[dict], list[dict], list[str]]:
    documents: list[dict] = []
    contents: dict[str, str] = {}
    issues: list[str] = []
    if not (root / "docs").is_dir():
        return [], [], ["missing docs/ root"]
    identities: set[str] = set()
    for file in sorted((root / "docs").rglob("*.md")):
        path = file.relative_to(root).as_posix()
        content = file.read_text(encoding="utf-8")
        try:
            meta = read_metadata(content, path)
        except ValueError as exc:
            issues.append(str(exc))
            continue
        if meta["id"] in identities:
            issues.append(f"duplicate document ID: {meta['id']}")
        identities.add(meta["id"])
        actual_domain = "docs" if path == "docs/README.md" else path.split("/")[1]
        if meta["domain"] != actual_domain:
            issues.append(f"{path}: metadata domain disagrees with directory")
        title = HEAD.search(content)
        documents.append({**meta, "path": path, "title": title.group(1).strip() if title else path})
        contents[path] = content
    by_path = {doc["path"]: doc for doc in documents}
    edges: list[dict] = []
    seen_edges: set[tuple[str, str]] = set()
    for path, content in contents.items():
        for witness in authored_links(content):
            target = (document_target(path, witness["target"])
                      if witness["syntax"] == "MARKDOWN"
                      else wiki_target(witness["target"]))
            if target is None:
                continue
            if target not in by_path:
                issues.append(f"{path}: broken document link {target} at line {witness['line']}")
                continue
            edge = by_path[path]["id"], by_path[target]["id"]
            if edge in seen_edges:
                continue
            seen_edges.add(edge)
            relation = "ROUTES_TO" if by_path[path]["role"] == "ROUTER" else "RELATES_TO"
            edges.append({
                "from": edge[0], "to": edge[1], "type": relation,
                "evidence": {"path": path, **witness},
            })
    edges.sort(key=lambda edge: (edge["from"], edge["to"], edge["type"]))
    return documents, edges, issues


def verify_knowledge(root: Path = ROOT) -> list[str]:
    documents, edges, issues = catalog(root)
    by_path = {doc["path"]: doc for doc in documents}
    for path in ROUTERS:
        if path not in by_path:
            issues.append(f"missing documentation router: {path}")
        elif by_path[path]["role"] != "ROUTER":
            issues.append(f"invalid router role: {path}")
    if "docs/README.md" not in by_path:
        return sorted(set(issues))
    adjacency: dict[str, list[str]] = {}
    by_id = {doc["id"]: doc for doc in documents}
    for edge in edges:
        # Neither a draft nor a retired/historical note can act as an active
        # document-routing bridge. Graph discovery may still show those edges.
        source, target = by_id[edge["from"]], by_id[edge["to"]]
        if any(item["lifecycle"] != "ACTIVE" or item["authority"] == "HISTORICAL"
               for item in (source, target)):
            continue
        adjacency.setdefault(edge["from"], []).append(edge["to"])
    seen: set[str] = set()
    queue = deque([by_path["docs/README.md"]["id"]])
    while queue:
        node = queue.popleft()
        if node in seen:
            continue
        seen.add(node)
        queue.extend(adjacency.get(node, []))
    for doc in documents:
        if doc["lifecycle"] == "ACTIVE" and doc["id"] not in seen:
            issues.append(f"document not reachable from canonical router: {doc['path']}")
    return sorted(set(issues))


def sections(text: str) -> list[tuple[str, str, int]]:
    headers = list(SECTION.finditer(_prose_text(text)))
    if not headers:
        return [("(document)", text, 1)]
    result = [("(intro)", text[:headers[0].start()], 1)]
    for index, header in enumerate(headers):
        stop = headers[index + 1].start() if index + 1 < len(headers) else len(text)
        result.append((header.group(2).strip(), text[header.start():stop], text.count("\n", 0, header.start()) + 1))
    return result


def retrieve(query: str, domain: str, root: Path = ROOT, limit: int = 8) -> list[dict]:
    if domain not in {"foundation", "knowledge", "system", "all"}:
        raise ValueError("retrieve only from a selected domain unless explicitly all")
    if limit < 1 or limit > 25:
        raise ValueError("retrieval limit must be 1..25")
    terms = WORDS.findall(query.lower())
    if not terms:
        raise ValueError("empty retrieval query")
    docs, _, errors = catalog(root)
    if errors:
        raise ValueError("invalid documentation catalog: " + "; ".join(errors[:3]))
    results = []
    for doc in docs:
        if domain != "all" and doc["domain"] != domain:
            continue
        if doc["lifecycle"] != "ACTIVE":
            continue
        content = (root / doc["path"]).read_text(encoding="utf-8")
        by_line = {s["line"]: s["id"] for s in derived_sections(doc, content)}
        for heading, body, line in sections(content):
            haystack = (doc["title"] + " " + heading + " " + body).lower()
            if not all(term in haystack for term in terms):
                continue
            score = sum(6 * int(term in doc["title"].lower()) + 4 * int(term in heading.lower()) + min(haystack.count(term), 5) for term in terms)
            results.append({
                "id": doc["id"], "sectionId": by_line.get(line, doc["id"]),
                "path": doc["path"], "authority": doc["authority"],
                "title": doc["title"], "section": heading, "line": line,
                "score": score, "excerpt": " ".join(body.split())[:240],
            })
    results.sort(key=lambda item: (-item["score"], item["path"], item["line"]))
    return results[:limit]



# These are derived lookup resources. Physical Markdown remains the sole owner.
PROOF_BOUNDARY = "DOCUMENT_NAVIGATION_ONLY_NOT_SOURCE_OR_RUNTIME_PROOF"


def derived_sections(doc: dict, text: str) -> list[dict]:
    occurrences: dict[str, int] = {}
    result: list[dict] = []
    for heading, _, line in sections(text):
        if heading in {"(intro)", "(document)"}:
            continue
        slug = "-".join(WORDS.findall(heading.lower()))[:80] or "section"
        occurrences[slug] = occurrences.get(slug, 0) + 1
        suffix = "" if occurrences[slug] == 1 else f"-{occurrences[slug]}"
        result.append({
            "id": f"{doc['id']}.section.{slug}{suffix}",
            "parentId": doc["id"], "path": doc["path"],
            "class": "DOCUMENT_SECTION", "domain": doc["domain"],
            "role": "SECTION", "authority": "DERIVED",
            "lifecycle": doc["lifecycle"], "title": heading, "line": line,
        })
    return result


def build_architecture_graph(root: Path = ROOT) -> tuple[dict[str, dict], list[dict], list[str]]:
    """Read current docs; no separate index, graph cache or external services."""
    documents, edges, issues = catalog(root)
    nodes = {doc["id"]: doc for doc in documents}
    for doc in documents:
        text = (root / doc["path"]).read_text(encoding="utf-8")
        for section in derived_sections(doc, text):
            if section["id"] in nodes:
                issues.append(f"duplicate derived section ID: {section['id']}")
            nodes[section["id"]] = section
            edges.append({
                "from": section["id"], "to": doc["id"], "type": "DERIVED_FROM",
                "evidence": {"path": doc["path"], "line": section["line"],
                             "syntax": "HEADING", "target": section["title"]},
            })
    edges.sort(key=lambda edge: (edge["from"], edge["to"], edge["type"]))
    return nodes, edges, issues


def resource_view(resource_id: str, root: Path = ROOT) -> dict:
    nodes, edges, issues = build_architecture_graph(root)
    if issues:
        raise ValueError("invalid architecture graph: " + "; ".join(issues[:3]))
    if resource_id not in nodes:
        return {"status": "UNKNOWN_RESOURCE", "id": resource_id,
                "nodes": [], "steps": [], "proofBoundary": PROOF_BOUNDARY}
    related = [e for e in edges if resource_id in (e["from"], e["to"])]
    related_ids = {resource_id}
    for edge in related:
        related_ids.update((edge["from"], edge["to"]))
    own = nodes[resource_id]
    return {
        "status": "FOUND", "resource": own,
        "nodes": [nodes[key] for key in sorted(related_ids)],
        "steps": related, "incoming": [e for e in related if e["to"] == resource_id],
        "outgoing": [e for e in related if e["from"] == resource_id],
        "sections": [nodes[e["from"]] for e in related if e["type"] == "DERIVED_FROM"
                     and e["to"] == resource_id],
        "proofBoundary": PROOF_BOUNDARY,
    }


def explain_resource_path(start: str, target: str, root: Path = ROOT, *,
                          max_hops: int = 4, allow_reverse: bool = False,
                          include_historical: bool = False) -> dict:
    if max_hops < 1 or max_hops > 6:
        raise ValueError("graph trace max_hops must be 1..6")
    nodes, edges, issues = build_architecture_graph(root)
    if issues:
        raise ValueError("invalid architecture graph: " + "; ".join(issues[:3]))
    common = {"from": start, "to": target, "maxHops": max_hops,
              "reverse": allow_reverse, "history": include_historical,
              "proofBoundary": PROOF_BOUNDARY}
    if start not in nodes or target not in nodes:
        return {**common, "status": "UNKNOWN_RESOURCE", "nodes": [], "steps": []}
    def exclusion_reason(node_id: str) -> str | None:
        node = nodes[node_id]
        parent_id = node.get("parentId")
        owner = nodes[parent_id] if parent_id is not None else node
        if owner["lifecycle"] == "DRAFT":
            return "DRAFT_EXCLUDED"
        if not include_historical and owner["lifecycle"] == "RETIRED":
            return "RETIRED_EXCLUDED"
        if not include_historical and owner["authority"] == "HISTORICAL":
            return "HISTORICAL_EXCLUDED"
        return None

    for endpoint in (start, target):
        reason = exclusion_reason(endpoint)
        if reason:
            return {**common, "status": "NO_PATH", "reason": reason,
                    "nodes": [], "steps": []}
    adjacency: dict[str, list[tuple[str, dict]]] = {}
    for edge in edges:
        adjacency.setdefault(edge["from"], []).append((
            edge["to"], {**edge, "traversal": "FORWARD"}))
        if allow_reverse:
            adjacency.setdefault(edge["to"], []).append((
                edge["from"], {**edge, "from": edge["to"], "to": edge["from"],
                               "traversal": "REVERSE"}))
    queue = deque([(start, [])])
    visited = {start}
    while queue:
        current, path = queue.popleft()
        if current == target:
            route = [start] + [edge["to"] for edge in path]
            return {**common, "status": "FOUND",
                    "nodes": [nodes[resource] for resource in route],
                    "steps": path}
        if len(path) >= max_hops:
            continue
        for next_id, edge in sorted(adjacency.get(current, []),
                                    key=lambda item: (item[0], item[1]["type"])):
            if next_id in visited:
                continue
            if exclusion_reason(next_id):
                continue
            visited.add(next_id)
            queue.append((next_id, path + [edge]))
    return {**common, "status": "NO_PATH", "reason": "NO_EXTRACTED_ROUTE_WITHIN_SCOPE",
            "nodes": [], "steps": []}


def mermaid_projection(view: dict) -> str:
    """Mermaid source only. Pele/Obsidian are not executed or installed."""
    nodes, edges = view.get("nodes", []), view.get("steps", [])
    if not nodes:
        return 'flowchart LR\n    unknown["NO_PATH_OR_UNKNOWN_RESOURCE"]'
    ids = {node["id"]: f"n{index}" for index, node in enumerate(nodes)}
    lines = ["flowchart LR"]
    for node in nodes:
        label = re.sub(r'[^a-zA-Z0-9 ./_-]', " ", node["title"])[:72]
        lines.append(f'    {ids[node["id"]]}["{label}"]')
    for edge in edges:
        relation = edge["type"] if edge.get("traversal") != "REVERSE" else "REVERSE_" + edge["type"]
        if edge["from"] in ids and edge["to"] in ids:
            lines.append(f'    {ids[edge["from"]]} -->|{relation}| {ids[edge["to"]]}')
    return "\n".join(lines)


def markdown_projection(view: dict) -> str:
    lines = ["# Repository architecture navigation", "",
             "Status: " + view["status"], "",
             "Proof boundary: " + view["proofBoundary"], "",
             "## Resource route", ""]
    for node in view.get("nodes", []):
        lines.append(f"- [[{node['path']}|{node['id']}]] ({node['authority']})")
    lines.extend(("", "## Source link witnesses", ""))
    for edge in view.get("steps", []):
        evidence = edge["evidence"]
        lines.append(f"- {edge['from']} -> {edge['to']} ({edge['type']}): "
                     f"{evidence['path']}:{evidence['line']} "
                     f"[{evidence['syntax']}: {evidence['target']}]")
    fence = chr(96) * 3
    lines.extend(("", "## Mermaid / Pele-compatible diagram", "", fence + "mermaid",
                  mermaid_projection(view), fence, ""))
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    opts = parser.add_mutually_exclusive_group(required=True)
    opts.add_argument("--check", action="store_true")
    opts.add_argument("--summary", action="store_true")
    opts.add_argument("--id")
    opts.add_argument("--trace", nargs=2, metavar=("FROM_ID", "TO_ID"))
    opts.add_argument("--query")
    parser.add_argument("--domain", choices=["foundation", "knowledge", "system"])
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--limit", type=int, default=8)
    parser.add_argument("--max-hops", type=int, default=4)
    parser.add_argument("--both-directions", action="store_true")
    parser.add_argument("--history", action="store_true")
    parser.add_argument("--format", choices=["json", "markdown", "mermaid"], default="json")
    args = parser.parse_args()
    if args.format != "json" and not (args.id or args.trace):
        parser.error("--format markdown/mermaid requires --id or --trace")
    if (args.both_directions or args.history or args.max_hops != 4) and not args.trace:
        parser.error("--both-directions, --history and --max-hops require --trace")
    if args.check:
        issues = verify_knowledge()
        print(json.dumps({"passed": not issues, "issues": issues}, indent=2))
        return int(bool(issues))
    if args.summary:
        nodes, edges, issues = build_architecture_graph()
        if issues:
            print(json.dumps({"passed": False, "issues": issues}, indent=2))
            return 1
        docs = [node for node in nodes.values() if node["class"] == "DOCUMENT"]
        print(json.dumps({
            "documents": len(docs), "sections": len(nodes) - len(docs),
            "relationships": len(edges),
            "byRelation": dict(sorted(Counter(edge["type"] for edge in edges).items())),
            "byDomain": dict(Counter(d["domain"] for d in docs)),
            "byAuthority": dict(Counter(d["authority"] for d in docs)),
            "proofBoundary": PROOF_BOUNDARY,
        }, indent=2))
        return 0
    if args.id:
        view = resource_view(args.id)
    elif args.trace:
        view = explain_resource_path(args.trace[0], args.trace[1],
                                     max_hops=args.max_hops,
                                     allow_reverse=args.both_directions,
                                     include_historical=args.history)
    else:
        if not args.all and not args.domain:
            parser.error("specify --domain to bound retrieval; use --all only for diagnostic broad search")
        try:
            results = retrieve(args.query, "all" if args.all else args.domain, limit=args.limit)
        except ValueError as exc:
            parser.error(str(exc))
        view = {"results": results}
    if args.format == "markdown":
        print(markdown_projection(view))
    elif args.format == "mermaid":
        print(mermaid_projection(view))
    else:
        print(json.dumps(view, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
