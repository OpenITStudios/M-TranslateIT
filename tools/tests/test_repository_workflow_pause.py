"""The dependency verification gate must preserve the owner's CI pause."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools import verify_repository

WORKFLOW = """name: Repository Verify
on:
  workflow_dispatch:
permissions:
  contents: read
jobs:
  verify:
    if: false # CI paused by owner
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - run: python tools/verify_repository.py
  second:
    if: false && (always()) # CI paused by owner
    timeout-minutes: 5
    steps:
      - run: echo paused
"""


class WorkflowPauseTests(unittest.TestCase):
    def inspect(self, source):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            (workflows / "repository-verify.yml").write_text(source, encoding="utf-8")
            with patch.object(verify_repository, "ROOT", root):
                errors = []
                verify_repository.check_workflows(errors)
            return errors

    def test_all_jobs_must_remain_paused(self):
        self.assertEqual(self.inspect(WORKFLOW), [])
        enabled = WORKFLOW.replace("if: false && (always())", "if: always()")
        self.assertTrue(any("job second must remain disabled" in e for e in self.inspect(enabled)))

    def test_automatic_triggers_remain_forbidden(self):
        automatic = WORKFLOW.replace("on:\n", "on:\n  push:\n    branches: [Local]\n")
        self.assertTrue(any("must not auto-trigger" in e for e in self.inspect(automatic)))

    def test_action_upgrades_still_require_immutable_pins(self):
        mutable = WORKFLOW.replace("3d3c42e5aac5ba805825da76410c181273ba90b1", "v7")
        self.assertTrue(any("mutable action ref" in e for e in self.inspect(mutable)))


if __name__ == "__main__":
    unittest.main()
