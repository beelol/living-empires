#!/usr/bin/env python3
"""Advisory ownership and integration locks shared by a clone's Git worktrees.

Requires Python 3.9+ and POSIX flock (macOS/Linux). No network calls, Git
mutations, credentials, dependency installation, or automatic claim expiry.
"""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import fnmatch
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


class CoordinationError(Exception):
    pass


def git(*args, cwd=None):
    result = subprocess.run(
        ["git", *args], cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise CoordinationError(os.fsdecode(result.stderr).strip())
    return os.fsdecode(result.stdout)


def locations():
    root = Path(git("rev-parse", "--show-toplevel").strip()).resolve()
    common = Path(git("rev-parse", "--git-common-dir", cwd=root).strip())
    if not common.is_absolute():
        common = root / common
    registry = common.resolve() / "agent-coordination"
    registry.mkdir(exist_ok=True)
    return root, registry


@contextmanager
def locked(path, nonblocking=False):
    with path.open("a+") as handle:
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | (fcntl.LOCK_NB if nonblocking else 0))
        except BlockingIOError:
            raise CoordinationError("Another process holds the local integration lock.")
        # The descriptor is passed to wrapped commands, so a killed wrapper does
        # not free the lock while its direct child is still running.
        yield handle


def read_claims(registry):
    claims = []
    for path in sorted(registry.glob("*.json")):
        try:
            claim = json.loads(path.read_text())
            required = {"id", "worktree", "acs", "affects", "paths", "base", "created"}
            if not isinstance(claim, dict) or not required <= claim.keys():
                raise ValueError("missing claim fields")
            if any(not isinstance(claim[key], list) for key in ("acs", "affects", "paths")):
                raise ValueError("invalid claim lists")
            claims.append(claim)
        except (ValueError, TypeError) as error:
            raise CoordinationError(f"Invalid claim {path}: {error}; reconcile it before continuing.")
    return claims


def atomic_write(path, value):
    descriptor, temporary = tempfile.mkstemp(prefix=".claim-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as handle:
            json.dump(value, handle, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def validate_paths(paths):
    for path in paths:
        parts = path.rstrip("/").split("/")
        if (not path or path.startswith("/") or "\\" in path or "\0" in path
                or any(part in ("", ".", "..", ".git") for part in parts)):
            raise CoordinationError(f"Use repository-relative paths without traversal or .git: {path!r}")
    return sorted(set(paths))


def conflict_prefix(pattern):
    """Reserve the literal directory preceding a glob, deliberately conservatively."""
    parts = pattern.rstrip("/").casefold().split("/")
    for index, part in enumerate(parts):
        if any(character in part for character in "*?["):
            return "/".join(parts[:index])
    return "/".join(parts)


def overlaps(first, second):
    first, second = conflict_prefix(first), conflict_prefix(second)
    return (not first or not second or first == second
            or first.startswith(second + "/") or second.startswith(first + "/"))


def owns(pattern, path):
    if any(character in pattern for character in "*?["):
        return fnmatch.fnmatchcase(path, pattern)
    prefix = pattern.rstrip("/")
    return path == prefix or path.startswith(prefix + "/")


def owned_claim(claims, stream_id, root):
    for claim in claims:
        if claim["id"] == stream_id:
            if claim["worktree"] != str(root):
                raise CoordinationError(f"Stream {stream_id} belongs to checkout {claim['worktree']}.")
            return claim
    raise CoordinationError(f"No active claim for {stream_id}.")


def create_claim(args, root, registry, claims):
    known = set(re.findall(r"\*\*([A-Z]+-\d+)\s+—", (root / "README.md").read_text()))
    unknown = (set(args.acs) | set(args.affects)) - known
    if unknown:
        raise CoordinationError("Unknown README criterion IDs: " + ", ".join(sorted(unknown)))
    paths = validate_paths(args.paths)
    for claim in claims:
        if claim["id"] == args.id:
            raise CoordinationError(f"Stream {args.id} already exists; do not overwrite active claims.")
        if claim["worktree"] == str(root):
            raise CoordinationError(f"Checkout already belongs to {claim['id']}; use an isolated worktree.")
        conflicts = [(left, right) for left in paths for right in claim["paths"] if overlaps(left, right)]
        if conflicts:
            raise CoordinationError(f"Path overlap with {claim['id']}: {conflicts}")
    claim = {
        "id": args.id, "worktree": str(root), "acs": sorted(set(args.acs)),
        "affects": sorted(set(args.affects)), "paths": paths,
        "base": git("rev-parse", "HEAD", cwd=root).strip(),
        "created": datetime.now(timezone.utc).isoformat(),
    }
    for other in claims:
        shared = (set(claim["acs"] + claim["affects"])
                  & set(other["acs"] + other["affects"]))
        if shared:
            print(f"Coordinate shared ACs with {other['id']}: {', '.join(sorted(shared))}", file=sys.stderr)
    atomic_write(registry / f"{args.id}.json", claim)
    print(json.dumps(claim, indent=2))


def changed_paths(root, base):
    # No rename detection means both removed and added paths are checked.
    paths = set()
    for args in (
        ("diff", "--name-only", "--no-renames", "-z"),
        ("diff", "--cached", "--name-only", "--no-renames", "-z"),
        ("ls-files", "--others", "--exclude-standard", "-z"),
    ):
        paths.update(filter(None, git(*args, cwd=root).split("\0")))
    if base:
        revision = git("rev-parse", "--verify", "--end-of-options", base + "^{commit}", cwd=root).strip()
        paths.update(filter(None, git(
            "diff", "--name-only", "--no-renames", "-z", revision, "HEAD", "--", cwd=root,
        ).split("\0")))
    return paths


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    claim = commands.add_parser("claim", help="reserve a stream's small path set")
    claim.add_argument("--id", required=True)
    claim.add_argument("--acs", nargs="+", required=True, help="primary README criterion IDs")
    claim.add_argument("--affects", nargs="+", default=[], help="other affected criterion IDs")
    claim.add_argument("--paths", nargs="+", required=True, help="quote wildcard patterns")
    commands.add_parser("list", help="list this clone's claims as JSON")
    release = commands.add_parser("release", help="release a claim from its owning checkout")
    release.add_argument("--id", required=True)
    check = commands.add_parser("check", help="reject changed paths outside the current claim")
    check.add_argument("--id", required=True)
    check.add_argument("--base", help="also check committed changes since this Git revision")
    lock = commands.add_parser("lock", help="run a command under the clone's integration lock")
    lock.add_argument("argv", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if hasattr(args, "id") and not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", args.id):
        raise CoordinationError("Stream IDs must be 1–64 lowercase letters, digits, underscores, or hyphens.")
    root, registry = locations()
    if args.command == "lock":
        command = args.argv[1:] if args.argv[:1] == ["--"] else args.argv
        if not command:
            raise CoordinationError("Provide a command after -- (for example: lock -- sh).")
        with locked(registry / "integration.lock", nonblocking=True) as handle:
            return subprocess.run(command, pass_fds=(handle.fileno(),)).returncode
    with locked(registry / "claims.lock"):
        claims = read_claims(registry)
        if args.command == "claim":
            create_claim(args, root, registry, claims)
        elif args.command == "list":
            print(json.dumps(claims, indent=2))
        else:
            claim = owned_claim(claims, args.id, root)
            if args.command == "release":
                (registry / f"{args.id}.json").unlink()
                print(f"Released {args.id}.")
            elif args.command == "check":
                paths = changed_paths(root, args.base)
                outside = sorted(path for path in paths if not any(owns(p, path) for p in claim["paths"]))
                if outside:
                    raise CoordinationError("Changes outside claim: " + ", ".join(outside))
                print(f"Ownership check passed for {len(paths)} changed paths; this does not verify behavior or AC completion.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (CoordinationError, OSError) as error:
        print(f"agent: {error}", file=sys.stderr)
        sys.exit(1)
