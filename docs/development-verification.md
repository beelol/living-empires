# Bevy / Lightyear development verification

Verified 2026-10-25 on this Mac. This is development-foundation evidence, not a
pass for the README's game acceptance criteria. All 122 game AC boxes remain unchecked.

## Current native foundation

- Rust 1.97.1, Bevy 0.20.0, Lightyear 0.31.0, embedded Lua 5.4.
- `cargo test --workspace`: **40 tests passed** (30 existing core, 10 native).
- `cargo test -p living-empires --no-default-features`: **10 passed**.
- `cargo clippy --workspace --all-targets -- -D warnings` and formatting check passed.
- Python coordination/deployment suite: **36 passed**.
- Real UDP integration: activation gate, protocol/session rejection, two identities,
  cross-client snapshots, duplicate/out-of-order movement, invalid Lua retention,
  valid replacement, durable crash recovery, and a persistent actual client through
  restart and lost acknowledgement. The same network loop powers the native view.
- Real commit supervisor smoke: committed snapshots only; invalid candidate kept
  old host; repaired Lua activated; native restart preserved identity/state; a
  second clone's remote push deployed without changing the local checkout.

[Deployment measurements](evidence/bevy-deployment-timings.json): local Lua commit
**0.706 s**, warm Rust change **3.881 s**, initial deployment using an existing Cargo
cache **5.382 s**, remote Lua push **0.833 s** with the test's 300 ms fetch interval.
Normal remote polling is 3 seconds plus network latency. These are observed runs,
not guarantees for cold builds, new dependencies, or arbitrary future changes.

[Native interaction records](evidence/bevy-native-checks.json) and
[actual Bevy screenshot](evidence/bevy-native-play.png): two native processes shared
the arena; keyboard and right-click commands changed saved authoritative position;
Lua changed movement, displayed name and fingerprint while both clients stayed
running. Direct file reload measured **47 ms** in that run. A real UDP probe joined
through the host's LAN IP. Closing a joined window left the host running; reopening
restored its identity. This verifies the local LAN address path, not another
physical device.

## Failures retained and corrected

- First persistent-client test panicked because Connect/Disconnect observer commands
  were deferred while the entity was inspected or removed. Explicit Bevy command
  flushes now preserve the lifecycle; persistent reconnect/lost-ack test passes.
- First raw-protocol test discarded replies by reading after Lightyear's Last-stage
  cleanup. An Update-stage collector fixed the test; no server workaround was used.
- Review found normal movement rejection permanently stopped the client. A separate
  MoveRejected response now keeps it usable and synchronizes its durable sequence.
- Native screenshot showed unsupported separator/ellipsis glyphs in the default
  font. ASCII text replaced them; the corrected actual render is retained above.
- Review found IPv6 wildcard health mapped to IPv4 loopback. The runner now targets
  IPv6 loopback; a regression check passes.

## Scope and remaining dependencies

Primary WORLD-01/03/05 and LIVE-08; affected LIVE-09/11, PLY-02/04 and AUD-03.
This demo uses reliable Lightyear commands/snapshots, not full replicated-ECS
prediction, game visibility rules, production admission, matchmaking or an approved
asset-release workflow. Server commits do not replace client executables; protocol
changes require coordinated client builds. The primitive arena is not release art.
A second physical device, the two-M2 reference load, and all full-game scenarios
remain unverified. No game AC is marked passed by these tests.

The Darwin debug linker reports that its large unwind section cannot fit compact
unwind offsets; builds and tests still succeed. The native app also warns about an
absent optional assets directory; this arena creates its meshes in code and its Lua
pack is loaded separately. Neither warning was hidden as a test failure.

## Historical HTTP foundation evidence

The following record predates the Bevy/Lightyear migration. Its browser screenshot
and timing numbers verify only the original core harness, not the current client.

# Development foundation verification

Verified locally on 2026-10-25, macOS arm64, Rust 1.89.0, Python 3.14.7.
The repository began at `9e616b8` on `origin/master`, containing the game checklist
only. The development foundation is integrated on `main`. No full-game acceptance
criterion is marked passed.

## Checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed (CLI options, content and real HTTP/world tests).
- `python3 -m unittest discover -s dev -p 'test_*.py' -v`: 35 tests passed.
- `python3 dev/smoke.py`: passed using a disposable local Git remote and the real
  Rust executable, including a second clone's push.
- Browser inspection: two independent browser origins joined the same host;
  movement from each appeared in both views. Restarting the host restored both
  commanders, and movement remained usable. [Screenshot](evidence/playground.png).
- The original 122 game ACs remain unchanged and unchecked in README.md.

Content checks exercise declared modules, dependency edits, malformed and missing
files, schema rejection, path traversal, symlink replacement, regular-file checks,
last-good rollback, memory/instruction constraints and bounded native operations.
Regression tests kill a subprocess on timeout so a broken execution limit cannot
hang the test suite indefinitely. Supported small table/string helpers still work.

Host checks exercise private session capabilities, no token leakage in shared
state, finite bounded movement, save-before-ack behavior, replayed requests across
restart, corrupt-save refusal, one save writer, failed-save behavior, activation
gating, same-origin JSON commands, input limits, stalled/chunked requests, delayed
packet bodies, and content-error recovery. An incomplete client request does not
block the authoritative world loop. A candidate cannot acknowledge a mutation
before the deployment runner commits promotion.

Supervisor checks exercise local/remote fast-forward selection, divergence and
rewind refusal, shared-worktree singleton locking, dirty-checkout isolation,
content-only activation without restart, bad content/build/probe/live startup,
rollback, marker-creation failure, crash recovery, build cancellation, malformed
health output, retrying immutable releases, and renames across the native/content
boundary. Claim-tool tests exercise conflicting ownership, worktree sharing,
concurrent claims, rename endpoints, uncommitted and committed scope checks, and
integration-lock exclusion.

## Measured iteration times

These are observations on this machine, not guaranteed upper bounds. Dependency
packages were already downloaded. The smoke test built in a fresh target directory
for its first deploy, then reused that cache. Its poll intervals were 100ms local
and 300ms remote; the normal runner defaults are 250ms and three seconds.

| Change | Observed elapsed time |
| --- | ---: |
| Direct Lua file save → host content version changes, three samples | 74.79 / 94.89 / 95.99 ms |
| Local Lua commit → healthy deployment | 0.543 s |
| Native source commit → build, probe, restart, healthy deployment | 2.524 s |
| Second clone's push → healthy Lua deployment, at 300ms fetch | 0.642 s |
| First build + deployment in fresh target directory | 14.429 s |

Raw results: [deployment](evidence/deployment-timings.json),
[direct reload](evidence/reload-timings.json). Earlier runs under concurrent test
load were slower (1.49s content / 7.84s warm native), so retain the distinction
between measured results and a hard latency guarantee.

## Deliberate limits

This is a LAN development playground with an embedded browser client. It does not
implement the 3D renderer, terrain, factions, economy, combat, private visibility,
AI services, asset production/review or coordinated large-asset client activation.
The browser receives authoritative positions and a content fingerprint; it does
not download Lua. Lua statics reset on reload while Rust-owned player state stays.

Physical second-device behavior, internet connectivity, M2 reference hardware,
50ms RTT/1% loss, full game performance, and production release approval have not
been audited. The two-client check used two browser origins on this Mac. Local
claims cannot coordinate separate clones by themselves. Native client HTML changes
require a browser refresh. The daemon must be started again after logout/reboot.
The small custom HTTP transport intentionally supports only bounded HTTP requests
with Connection: close; it is not a general-purpose public web server.

Independent review found and drove fixes for native Lua operations bypassing
instruction hooks, FIFO reads, pre-promotion save rollback, Git rename
classification, slow request handling, and the default LAN bind. The final socket
regression also covers macOS accepted-socket nonblocking inheritance.
