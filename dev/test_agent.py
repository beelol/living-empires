"""Behavior tests for local stream coordination; uses real temporary Git repos."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("agent.py")


class AgentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / "repo"
        self.repo.mkdir()
        self.git("init", "-b", "main")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Test")
        (self.repo / "README.md").write_text(
            "- [ ] **WORLD-01 — Authority.**\n- [ ] **SET-01 — Build.**\n"
        )
        (self.repo / "old.txt").write_text("old\n")
        self.git("add", "README.md", "old.txt")
        self.git("commit", "-m", "fixture")

    def git(self, *args, cwd=None):
        return subprocess.run(
            ["git", *args], cwd=cwd or self.repo, check=True,
            text=True, capture_output=True,
        ).stdout.strip()

    def run_agent(self, *args, cwd=None, ok=True):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), *args], cwd=cwd or self.repo,
            text=True, capture_output=True,
        )
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def worktree(self, name):
        path = Path(self.temp.name) / name
        self.git("worktree", "add", "-b", name, str(path))
        return path

    def claim(self, stream="one", paths=("server/",), cwd=None, **kwargs):
        return self.run_agent(
            "claim", "--id", stream, "--acs", "SET-01", "--paths", *paths,
            cwd=cwd, **kwargs,
        )

    def test_linked_worktrees_share_claims_and_reject_ancestor_globs(self):
        # A per-worktree registry or literal-only matching would allow a collision.
        self.claim(paths=("server/economy.py",))
        other = self.worktree("other")
        result = self.claim("two", ("server/**",), cwd=other, ok=False)
        self.assertIn("overlap", result.stderr)
        claims = json.loads(self.run_agent("list", cwd=other).stdout)
        self.assertEqual([claim["id"] for claim in claims], ["one"])

    def test_disjoint_paths_and_related_acs_can_be_claimed(self):
        self.claim()
        other = self.worktree("other")
        result = self.claim("two", ("client/",), cwd=other)
        self.assertIn("SET-01", result.stderr)
        self.assertEqual(len(json.loads(self.run_agent("list").stdout)), 2)

    def test_release_requires_the_owning_checkout(self):
        self.claim()
        other = self.worktree("other")
        self.run_agent("release", "--id", "one", cwd=other, ok=False)
        self.assertEqual(len(json.loads(self.run_agent("list").stdout)), 1)
        self.run_agent("release", "--id", "one")
        self.assertEqual(json.loads(self.run_agent("list").stdout), [])

    def test_claim_validates_ac_ids_and_relative_paths(self):
        for args in (
            ("--acs", "WORLD-99", "--paths", "server/"),
            ("--acs", "SET-01", "--affects", "UNKNOWN-01", "--paths", "server/"),
            ("--acs", "SET-01", "--paths", "../outside"),
            ("--acs", "SET-01", "--paths", "/absolute"),
            ("--acs", "SET-01", "--paths", ".git/"),
        ):
            with self.subTest(args=args):
                self.run_agent("claim", "--id", "one", *args, ok=False)
        self.assertEqual(json.loads(self.run_agent("list").stdout), [])

    def test_check_includes_staged_unstaged_and_untracked_paths(self):
        self.claim(paths=("server/",))
        (self.repo / "server").mkdir()
        (self.repo / "server" / "owned.py").write_text("owned\n")
        self.run_agent("check", "--id", "one")
        (self.repo / "new.txt").write_text("new\n")
        (self.repo / "old.txt").write_text("changed\n")
        self.git("add", "new.txt")
        result = self.run_agent("check", "--id", "one", ok=False)
        self.assertIn("new.txt", result.stderr)
        self.assertIn("old.txt", result.stderr)

    def test_check_compares_commits_to_explicit_base(self):
        self.claim(paths=("server/",))
        base = self.git("rev-parse", "HEAD")
        (self.repo / "old.txt").write_text("changed\n")
        self.git("add", "old.txt")
        self.git("commit", "-m", "outside scope")
        self.run_agent("check", "--id", "one")
        result = self.run_agent("check", "--id", "one", "--base", base, ok=False)
        self.assertIn("old.txt", result.stderr)

    def test_check_includes_both_ends_of_rename(self):
        self.claim(paths=("server/",))
        (self.repo / "server").mkdir()
        self.git("mv", "old.txt", "server/new.txt")
        result = self.run_agent("check", "--id", "one", ok=False)
        self.assertIn("old.txt", result.stderr)

    def test_check_glob_does_not_own_entire_conservative_conflict_prefix(self):
        self.claim(paths=("server/*.py",))
        (self.repo / "server").mkdir()
        (self.repo / "server" / "secret.txt").write_text("outside\n")
        result = self.run_agent("check", "--id", "one", ok=False)
        self.assertIn("server/secret.txt", result.stderr)

    def test_one_stream_per_checkout_and_no_silent_claim_overwrite(self):
        self.claim()
        self.claim("two", ("client/",), ok=False)
        self.claim("one", ("client/",), ok=False)
        claims = json.loads(self.run_agent("list").stdout)
        self.assertEqual(claims[0]["paths"], ["server/"])

    def test_parallel_claim_requests_cannot_both_win(self):
        other = self.worktree("other")
        commands = [
            subprocess.Popen(
                [sys.executable, str(SCRIPT), "claim", "--id", name,
                 "--acs", "SET-01", "--paths", "server/"],
                cwd=directory, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            for name, directory in [("one", self.repo), ("two", other)]
        ]
        for process in commands:
            process.communicate(timeout=10)
        self.assertEqual(sorted(p.returncode for p in commands), [0, 1])

    def test_integration_lock_is_shared_and_preserves_command_status(self):
        other = self.worktree("other")
        marker = Path(self.temp.name) / "ready"
        release = Path(self.temp.name) / "release"
        process = subprocess.Popen(
            [sys.executable, str(SCRIPT), "lock", "--", sys.executable, "-c",
             "import pathlib,time,sys; pathlib.Path(sys.argv[1]).touch(); "
             "exec('while not pathlib.Path(sys.argv[2]).exists(): time.sleep(0.02)')",
             str(marker), str(release)], cwd=self.repo,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        try:
            import time
            deadline = time.monotonic() + 5
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue(marker.exists(), "locked command did not start")
            result = self.run_agent(
                "lock", "--", sys.executable, "-c", "pass", cwd=other, ok=False,
            )
            self.assertIn("integration", result.stderr)
        finally:
            release.touch()
            process.communicate(timeout=5)
        result = self.run_agent(
            "lock", "--", sys.executable, "-c", "raise SystemExit(7)", ok=False,
        )
        self.assertEqual(result.returncode, 7)


if __name__ == "__main__":
    unittest.main()
