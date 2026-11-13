"""Regression checks for TranslateIT GitHub source-only/no-cloud policy."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_repository


class GitHubSourceOnlyGovernanceTests(unittest.TestCase):
    def test_current_candidate_and_skill_contracts(self):
        errors: list[str] = []
        verify_repository.check_chatgpt_execution_policy(errors)
        self.assertEqual(errors, [])

    def test_positive_cloud_development_language_is_rejected(self):
        read = verify_repository.text

        def incorrect(path: str) -> str:
            content = read(path)
            if path == "README.md":
                return content + "\nUse GitHub/cloud to run application tests.\n"
            return content

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=incorrect):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("implies cloud development" in issue for issue in errors))

    def test_hosted_execution_permission_regression_is_rejected(self):
        read = verify_repository.text

        def incorrect(path: str) -> str:
            content = read(path)
            if path == ".agents/permissions/permission-policy.json":
                return content.replace('"ci-dispatch",', "", 1)
            return content

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=incorrect):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("no-cloud permission actions" in issue for issue in errors))

    def test_removing_no_user_pc_rule_is_rejected(self):
        real_text = verify_repository.text

        def invalid(rel: str) -> str:
            value = real_text(rel)
            if rel == ".agent-standard/project-policy.md":
                return value.replace("Never ask the user to run", "Ask the user to run", 1)
            return value

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=invalid):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("Never ask the user to run" in error for error in errors))

    def test_removing_candidate_admission_is_rejected(self):
        real_text = verify_repository.text

        def invalid(rel: str) -> str:
            value = real_text(rel)
            if rel == "docs/knowledge/operations/target-windows-performance.md":
                return value.replace("## Candidate admission", "## Premature manual testing", 1)
            return value

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=invalid):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("## Candidate admission" in error for error in errors))


    def test_agent_rules_cannot_depend_on_another_product_repository(self):
        original = verify_repository.text

        def linked(rel: str) -> str:
            content = original(rel)
            if rel == "AGENTS.md":
                return content + "\nhttps://github.com/OpenITStudios/M-LazyDesigner/blob/main/AGENTS.md\n"
            return content

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=linked):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("must not depend on M-LazyDesigner" in issue for issue in errors))

    def test_agent_rules_missing_method_is_rejected(self):
        original = verify_repository.text

        def missing(rel: str) -> str:
            content = original(rel)
            if rel == ".agent-standard/methods.json":
                return content.replace('"repository": "cursor/plugins"', '"repository": "bogus/plugins"', 1)
            return content

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=missing):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("Agent Rules missing reviewed source: cursor/plugins" in issue for issue in errors))


if __name__ == "__main__":
    unittest.main()
