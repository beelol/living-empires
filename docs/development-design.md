# Fast local development foundation

This implements the user's requested iteration loop, not the entire game. The
canonical acceptance criteria remain in the root README, with context from the
[source Page](https://chatgpt.com/space/page_9d7940a6aa5c8191bc081c561105cfbb).
No full-game criterion is marked passed by this foundation.

## Chosen approach

Use a Rust workspace with Bevy 0.20, Lightyear 0.31 and embedded Lua 5.4 (`mlua`).
The `empires` executable has host/play/join modes. `play` keeps the Bevy window on
the main thread and starts the same executable as a host child, or joins the local
deployment runner. The host uses Bevy's headless schedule with real Lightyear UDP
Netcode connections and ordered reliable messages. It reuses the tested Rust world
and Lua core. TCP health is an operator endpoint; player commands travel through
Lightyear. Default builds include native graphics; deployments build the headless
feature set to keep the iteration loop small.

Session capabilities bind each connection to its owned player. Rust validates and
persists moves before acknowledgement. Snapshots drive the native 3D arena. The
client persists identity, refreshes expired connection tokens on reconnect, and
reconciles sequence numbers with the authority. Shared development transport keys
are limited to trusted networks. Protocol v1 is a development contract, not full
ECS replication, prediction, visibility filtering or production admission control.

Immutable content packs declare their files and format in a manifest. Load and
validate a new Lua VM before replacing the active one. Rust retains player state.
Limit Lua memory and execution; exclude filesystem, process and native module
access. Reject invalid paths, missing dependencies, malformed content and
incompatible formats. A 100ms polling loop suits this small bounded pack and
detects atomic directory/symlink replacement consistently across operating systems.
Large assets will need asynchronous streaming and a client activation protocol.

A Python stdlib supervisor watches local `main` and polls `origin/main`, builds
immutable commit snapshots with a shared Cargo cache, and deploys only validated
results. A local Git hook can wake it immediately. Lua-only commits switch a
content pointer without restarting the host. Native changes require a build and
health-checked restart, with durable state and rollback. A candidate rejects
mutating commands until a per-process activation marker is published after the
healthy release record, so rollback cannot discard acknowledged candidate actions. Remote polling is needed
because another computer's Git commit cannot invoke a hook on this one.

Alternatives considered: Watchexec is useful for a simple edit/restart loop, but
does not supply commit isolation, promotion or rollback. A GitHub webhook needs a
reachable receiver/tunnel and credentials, so bounded polling is the initial
default. Container rebuilds add work to this local iteration path. The selected
approach keeps the toolchain Rust + Python + Git; there is no external Lua install.

Seconds are the content-deployment target, not a promise for cold Cargo builds,
network outages or arbitrary future native changes. Record actual measurements.

## Contracts protected

LIVE-06–13: validation, retained previous content and version fingerprints are
foundations only; independent art review and multi-client asset readiness remain
required later. WORLD-01–05: Rust owns state, rejects unknown sessions and persists
acknowledged movement; fog of war, full game commands, durable simulation and
cross-device audit remain later work. AUD-03/04: revisions and versions must be
observable and game ACs must retain honest evidence requirements.

## Implementation and verification plan

- [x] Content loader: write failing tests for missing files, incompatible formats,
  path traversal, bad Lua, execution limits and rollback; implement atomic reload;
  run Rust tests.
- [x] Host/client: test session ownership, persistence and rejected input; implement
  host/play/join with a minimal shared view; run actual two-client HTTP checks.
- [x] Deployment: test branch selection, isolated candidates, lock contention and
  failed promotion; implement hook/poll runner; measure real Lua and Rust changes.
- [x] Coordination: add AGENTS.md and local claim tooling; test overlapping paths,
  linked worktrees and ownership; document cross-machine coordination limits.
- [x] Integration: run all Rust/Python checks, inspect the browser playground,
  independently review, record evidence and exact start/stop/join instructions.

Each task has separately owned files. Shared Cargo/API contracts are coordinated
by the parent agent. The Lua/persistence core remains under `dev/runtime/`; the native shell is under
`game/`. Existing README criteria are preserved verbatim. Historical HTTP evidence
does not verify the Bevy migration; current native evidence is recorded separately.

## Native migration stream

Primary: WORLD-01/03/05, LIVE-08. Affected: LIVE-09/11, PLY-02/04, AUD-03.
Root owns manifests, protocol v1, CLI, deployment runner, integration tests and docs.
The isolated native-server stream owns `game/src/server.rs` and `admin.rs`; the
native-client stream owns `client.rs` and `visual.rs`. Both depend on root's stable
protocol and the existing World's saved-before-ack API. Neither changes saved-world
format or approves content for LIVE release gates. Integration preserves the new
repository's README history and adds one squashed implementation commit on main.

Verification: real UDP connect, two identities, reconnect, duplicate sequence,
activation gate, bad protocol/unauthenticated moves, Lua replacement/rejection,
commit-triggered deployments, native window interaction and retained state.
