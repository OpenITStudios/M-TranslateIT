"""Document architecture: stable identities, explicit edges, retrieval proof boundaries."""
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_knowledge import (  # noqa: E402
    read_metadata, document_target, retrieve, verify_knowledge, catalog,
    authored_links, build_architecture_graph, resource_view, explain_resource_path,
    mermaid_projection, markdown_projection,
)


class KnowledgeTests(unittest.TestCase):
    def test_every_active_document_is_routed(self):
        self.assertEqual(verify_knowledge(), [])

    def test_search_is_domain_scoped_and_provenance_preserving(self):
        matches = retrieve("Meeting", "foundation", limit=6)
        self.assertTrue(matches)
        self.assertTrue(all(x["path"].startswith("docs/foundation/") for x in matches))
        self.assertTrue(all(x["authority"] == "CANONICAL" for x in matches))

    def test_system_domain_is_routed_and_ranked(self):
        matches = retrieve("Zero-Waste", "system", limit=6)
        self.assertTrue(matches)
        self.assertTrue(all(x["path"].startswith("docs/system/") for x in matches))
        docs, _, errors = catalog()
        self.assertEqual(errors, [])
        self.assertTrue(any(x["id"] == "document.system.application-runtime" for x in docs))
        self.assertFalse(any(x["path"].startswith("planning/") for x in docs))

    def test_planning_is_not_a_second_document_catalog(self):
        self.assertIsNone(document_target("docs/knowledge/README.md", "../../planning/development.md"))

    def test_id_does_not_change_with_physical_path(self):
        header = "---\nid: document.knowledge.continuity\nclass: DOCUMENT\ndomain: knowledge\nrole: GUIDE\nauthority: CANONICAL\nlifecycle: ACTIVE\n---\n"
        self.assertEqual(read_metadata(header, "docs/knowledge/renamed.md")["id"], "document.knowledge.continuity")

    def test_unrouted_document_is_detected(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "docs/knowledge").mkdir(parents=True)
            def doc(id_, domain, role):
                return f"---\nid: {id_}\nclass: DOCUMENT\ndomain: {domain}\nrole: {role}\nauthority: CANONICAL\nlifecycle: ACTIVE\n---\n# Example\n"
            (root / "docs/README.md").write_text(doc("document.docs.router", "docs", "ROUTER"), encoding="utf-8")
            (root / "docs/knowledge/isolated.md").write_text(doc("document.knowledge.isolated", "knowledge", "GUIDE"), encoding="utf-8")
            self.assertTrue(any("not reachable" in x for x in verify_knowledge(root)))

    def test_external_links_are_not_invented_edges(self):
        self.assertIsNone(document_target("docs/knowledge/README.md", "https://example.com/docs.md"))
        self.assertIsNone(document_target("docs/README.md", "../AGENTS.md"))
        self.assertEqual(document_target("docs/knowledge/README.md", "../foundation/README.md"), "docs/foundation/README.md")


    def test_fenced_inline_comment_and_image_links_are_not_graph_edges(self):
        markdown = (
            "[real](./target.md)\n"
            + chr(96) + "[fake](./hidden.md)" + chr(96) + "\n"
            + "![picture](./image.md)\n"
            + "<!-- [fake](./comment.md) -->\n"
            + chr(96) * 3 + "markdown\n[not-real](./fence.md)\n" + chr(96) * 3 + "\n"
            + "[[docs/system/owner.md|owner]]\n"
        )
        links = authored_links(markdown)
        self.assertEqual([item["target"] for item in links],
                         ["./target.md", "docs/system/owner.md"])
        self.assertEqual([item["line"] for item in links], [1, 8])

    def test_typed_trace_has_exact_witnesses_and_no_guessed_reverse(self):
        with TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "docs/system").mkdir(parents=True)
            def md(identity: str, domain: str, role: str, body: str) -> str:
                return ("---\nid: " + identity + "\nclass: DOCUMENT\ndomain: "
                        + domain + "\nrole: " + role
                        + "\nauthority: CANONICAL\nlifecycle: ACTIVE\n---\n"
                        + "# Resource\n" + body)
            (root / "docs/README.md").write_text(md(
                "document.docs.router", "docs", "ROUTER",
                "\n[system](./system/README.md)\n"), encoding="utf-8")
            (root / "docs/system/README.md").write_text(md(
                "document.system.router", "system", "ROUTER",
                "\n[[docs/system/owner.md|owner]]\n"), encoding="utf-8")
            (root / "docs/system/owner.md").write_text(md(
                "document.system.owner", "system", "ARCHITECTURE",
                "\n## Contract\nDetails\n## Contract\nMore details\n"), encoding="utf-8")
            nodes, edges, errors = build_architecture_graph(root)
            self.assertEqual(errors, [])
            self.assertIn("document.system.owner.section.contract-2", nodes)
            self.assertEqual(sum(edge["type"] == "DERIVED_FROM" for edge in edges), 2)
            forward = explain_resource_path("document.docs.router", "document.system.owner", root)
            self.assertEqual(forward["status"], "FOUND")
            self.assertEqual([edge["type"] for edge in forward["steps"]],
                             ["ROUTES_TO", "ROUTES_TO"])
            self.assertEqual(forward["steps"][1]["evidence"]["line"], 11)
            self.assertEqual(forward["steps"][1]["evidence"]["syntax"], "OBSIDIAN_WIKILINK")
            self.assertEqual(explain_resource_path(
                "document.system.owner", "document.docs.router", root)["status"], "NO_PATH")
            reverse = explain_resource_path(
                "document.system.owner", "document.docs.router", root,
                allow_reverse=True)
            self.assertEqual(reverse["status"], "FOUND")
            self.assertTrue(all(e["traversal"] == "REVERSE" for e in reverse["steps"]))
            self.assertEqual(explain_resource_path(
                "document.docs.router", "document.system.owner", root,
                max_hops=1)["status"], "NO_PATH")
            self.assertEqual(explain_resource_path(
                "document.docs.router", "document.system.missing", root)["status"],
                "UNKNOWN_RESOURCE")
            view = resource_view("document.system.owner", root)
            self.assertEqual(len(view["sections"]), 2)
            self.assertIn("[[docs/system/owner.md|document.system.owner]]",
                          markdown_projection(forward))
            self.assertIn("flowchart LR", mermaid_projection(forward))

    def test_search_exposes_derived_section_address_without_new_authority(self):
        matches = retrieve("Application Runtime", "system", limit=10)
        self.assertTrue(matches)
        self.assertTrue(all(item["sectionId"].startswith(item["id"]) for item in matches))
        self.assertTrue(all(item["authority"] == "CANONICAL" for item in matches))


if __name__ == "__main__":
    unittest.main()
