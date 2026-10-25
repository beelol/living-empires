# Local deployment loop

Run these commands from the repository root. Python 3.10 or newer, Git, Cargo, and the Rust toolchain are required. The runner uses only Python's standard library and supports macOS/Linux.

```sh
python3 dev/runner.py start
python3 dev/runner.py status
python3 dev/runner.py stop
```

`start` detaches the runner and writes `.local-deploy/runner.log`. `run` keeps it in the foreground. The Bevy/Lightyear host listens on UDP 4000; TCP `http://localhost:4000/health` reports status. Run `cargo run -p living-empires -- play` locally or `cargo run -p living-empires -- join HOST_IP:4000` on another device. Keep this development host on a trusted network: its shared transport key is not production admission control. Session capabilities still authorize world actions.

The integration branch is **main**. The runner checks `refs/heads/main` every 250 ms and fetches `origin/main` about every three seconds, using a private Git ref. It never silently switches to `master`. If this checkout uses another integration branch, choose it explicitly:

```sh
python3 dev/runner.py start --branch master
```

If both local and remote heads exist, the runner selects the newer head only when one is an ancestor of the other. Divergent heads and rewinds from the last healthy deployment stop promotion and appear in `status`; the current healthy host continues. Merge or fast-forward the integration branch to resolve divergence. Branch commits from agents become eligible only after integration into the selected branch. A remote fetch outage is recorded; local commits and the last fetched remote ref remain available.

The runner never resets, stashes, cleans, switches, or edits an agent checkout. Each candidate comes from `git archive` at one exact commit, in a read-only snapshot. Dirty and untracked files are excluded. A single advisory lock per deployment directory prevents concurrent runners. All worktrees use the common repository's `.local-deploy` directory by default, so twenty agents can work independently while one runner promotes their integrated commits. Use separate Git worktrees for agents and serialize integration into main; the runner does not merge competing agent branches.

## What updates during play

A commit changing only `dev/content/` is checked by the running version's `check-content` command. A successful check produces the expected content fingerprint. The runner switches `current-content` atomically, waits for `/health` to report that fingerprint, and rolls the pointer back if activation fails. The process and persistent world stay alive. The health `accepting_commands` field reports whether the activation gate is open. The `release` field identifies the binary commit; after hot reload it can differ from the latest content commit shown in runner status.

Every other commit gets an incremental Rust build with `cargo build --locked -p living-empires --bin empires --no-default-features`. The Cargo target directory is shared outside the snapshots. The runner copies the compiled executable into a release directory and validates its exact content. It then starts a temporary localhost host with a copy of the saved world and checks its health while the old host remains live. Only then does it restart the live host on the same address. Every candidate starts behind an activation gate: health checks work, but player join and move requests receive a retryable response. The runner verifies the gated host, records the active release, then opens its gate. A failed live startup restores the previous executable, content, and saved-world backup before any player commands can be acknowledged. Once the gate opens, the backup can never overwrite newly acknowledged actions. Clients briefly reconnect for Rust changes. This is development deployment, not a zero-downtime production upgrade system.

Local content commits should normally appear within a second. Remote-only commits add the fetch interval and network latency. A warm Rust build can take seconds; a first build, dependency changes, a slow machine, or network downloads can take substantially longer. Read actual activation durations in `runner.log`. No fixed “every Rust change in seconds” guarantee is possible.

## Controls and recovery

```sh
# Foreground, custom address and polling frequency:
python3 dev/runner.py run --bind 127.0.0.1:4001 --poll-interval 0.25 --fetch-interval 3

# Optional lightweight commit notification; polling works without it:
python3 dev/runner.py install-hook

# Verify the deployment behavior with temporary repositories and local hosts:
python3 -m unittest discover -s dev -p test_runner.py -v
```

The optional post-commit hook only touches a wake file in the common repository's `.local-deploy` directory; the runner checks that hint every 50 ms while idle. It does not build, fetch, or block on another runner. Existing hooks or a configured `core.hooksPath` are preserved: the installer refuses to overwrite them. Chain `dev/hooks/post-commit` from your existing hook if desired. Polling remains the fallback, including for merges and commits created without hooks. Custom `--deploy-dir` deployments rely on polling unless their wake file is touched directly.

`status` prints the active content commit, binary revision, content fingerprint, host PID, current phase, last deployment error, and remote-fetch error. Build and host logs live in `.local-deploy/logs/`. A rejected commit is not retried every tick; fix it in a new commit, or restart the runner to retry a transient build failure. The next start restores the last healthy release before checking for newer commits. Restart the runner after changing `dev/runner.py` itself; it does not replace its own running process. The detached runner continues after the terminal closes, but does not install a login service or survive a reboot automatically. `stop` requests orderly shutdown and cancels an in-progress build. Unexpected host exits trigger bounded retries of the last validated release; five repeated crashes without 30 seconds of stable serving stop retries and remain visible for diagnosis.

The persistent state file is `.local-deploy/world.json`. Candidate probes use separate copies and never write that file. Snapshots, release executables, logs, and the shared Cargo cache are retained for inspection; there is no destructive automatic cleanup. Stop the runner before doing any manual cleanup, and retain the state file plus the release referenced by `active.json`. If the runner itself is killed with SIGKILL, inspect any orphaned host before starting another runner; the runner never kills an unverified process just because it occupies the port.

This runner deploys trusted committed repository code, including build scripts and Lua. Content validation is not a security boundary against a malicious contributor. The actual runtime determines which Lua changes are compatible and which saved-world formats it can load. A process restart cannot promise backward compatibility for an arbitrary schema migration.
