# Fast local development

A native Rust Bevy/Lightyear + Lua **development playground** for Living Empires.
This is a foundation for the game. The [game acceptance criteria](../README.md)
remain the product contract. See [AGENTS.md](../AGENTS.md) before changing code.

## Start playing

This foundation uses **Rust 1.97.1, Bevy 0.20, Lightyear 0.31 and embedded Lua 5.4**.
Install Rust via rustup, Git and Python 3.10+. `rust-toolchain.toml` pins the compiler.
The initial dependency download/build takes longer than subsequent changes.

```sh
cargo run -p living-empires --bin empires -- play
```

This opens a native Bevy window and starts an authoritative Lightyear host at
`0.0.0.0:4000`. If the deployment runner already owns that address, `play` joins it.
Closing the window stops only a host started by that play command. Use WASD
or right-click movement; the window lists camera controls. Build this project on
a second device, then join the address printed in the host startup log:

```sh
cargo run -p living-empires --bin empires -- join 192.168.1.25:4000
```

Replace the example IP with your host's address. Both devices need a reachable LAN
or private VPN, and the host firewall must allow **UDP 4000**. TCP 4000 serves
health/status only. This development setup uses a shared Netcode transport key
and persisted application session capabilities. Production accounts, a trusted
token issuer, discovery, NAT traversal and matchmaking remain future work.

For a headless host or a second local experiment:

```sh
cargo run -p living-empires --no-default-features -- host --bind 0.0.0.0:4001 \
  --content dev/content --state .local/experiment.json
```

Each world needs its own save path. Only one host may hold a save at a time.
Use separate `--session .local/player-two.json` paths for separate players on one
computer. Session files contain identity capabilities; do not share them.
The host advertises its default LAN interface automatically. For a VPN or another
interface, add `--advertise YOUR_REACHABLE_IP:4001` (repeatable), or bind that exact
IP. Lightyear validates the advertised address during connection. The runner can
also use an exact address with `--bind YOUR_REACHABLE_IP:4000`.

## Gemini diplomacy demo

Put the host's key in `.env.local` beside the root `Cargo.toml` (copy
`.env.example`). This file is ignored by Git; never put a real key in the example.

```dotenv
GEMINI_API_KEY=your-key-here
GEMINI_MODEL=gemini-4-argon
DIPLOMACY_INTERVAL_SECONDS=10
DIPLOMACY_MAX_CALLS=120
```

The **Rust host** reads this file; no shell `source` step is needed. Existing
environment variables override file settings. `GOOGLE_API_KEY` is also accepted.
Set `ARGON_ENV_FILE=/absolute/path/.env.local` when launching elsewhere. The
deployment runner automatically supplies its repository's `.env.local` path, so
credentials stay outside release snapshots. Only the host needs the key.

Start `empires play` or `empires host` normally, then open
<http://127.0.0.1:4000/diplomacy> on the host computer (use your host's port).
Three seeded factions take turns with separate observations and histories. They
can send messages, change their own stance, offer a five-minute non-aggression
pact, and accept or decline the exact offer version. Offers expire after two
minutes. The Rust authority validates each result and saves it before displaying
success. Model calls run off the game loop. No key means waiting, not fake replies.

The host picks up a newly saved key, model, interval or call limit between calls.
Argon is the default; `GEMINI_MODEL=gemini-3.8-flash` can select Flash. Each HTTP attempt
counts toward the durable call limit, including failures; after the limit, the
loop pauses until you increase it. Calls time out after 20 seconds, failures
back off, and native movement continues. Removing the key pauses new requests.

The local operator page shows private observations and decision evidence, so
only localhost can read it. Player/network snapshots do not include these records.
The separate `<world-save>.diplomacy.json` retains faction identities, offers,
pacts and all bounded attempt/outcome evidence across host restarts. This demo
uses wall-clock pact deadlines. It does not yet connect combat enforcement,
territory, inventory, physical deliveries or generated leaders, and is not a
full-game acceptance pass.

## Edit Lua without a build

Change `distance` in [content/rules.lua](content/rules.lua) and save. The running
host checks the bounded content pack every 100ms and activates a valid change.
Player identities and positions stay in Rust. The displayed content fingerprint
changes; movement uses the new rule. Invalid edits retain the previous version
and expose the failure. Fix the file to retry. No Rust compilation is involved.

[content/manifest.json](content/manifest.json) declares every file. Add a Lua file
there, then load it with `require('module')`; `rules/helpers.lua` is named
`rules.helpers`. Only declared files are accessible. Each rules entry returns:

```lua
return {
  schema_version = 1,
  world_name = 'My development world',
  step = function(x, y, dx, dy)
    return x + dx * 12, y + dy * 12
  end,
}
```

The pack allows 128 files, 1 MiB per file and 4 MiB total. The Lua VM has an 8 MiB
memory limit and an instruction budget. Filesystem/process/native-library APIs
are unavailable, as are Lua's native pattern-search APIs (`find`, `match`, `gmatch`,
`gsub`), which cannot be interrupted by Lua instruction hooks. Native repetition
and table operations are restricted to 4,096 elements/counts and plain tables.
This is a limited
development rules API, not a general-purpose hostile-code isolation service.
Movement proposals must be finite and stay within Rust's bound.
Declared non-Lua files contribute to validation and the fingerprint; this bootstrap
does not yet stream models/textures to clients. Larger assets need a future
asynchronous loader and compatibility/activation handshake (LIVE-08–13).

```sh
cargo run -p living-empires --no-default-features -- check-content --content dev/content
```

## Automatically deploy commits from main

Stop a manually started host on port 4000 first. The deployment runner owns its
host and independent world save. It does not deploy uncommitted working files.

```sh
python3 dev/runner.py install-hook
python3 dev/runner.py start
python3 dev/runner.py status
```

Then run `cargo run -p living-empires -- play`. Local `main` commits are detected within about
250ms, and remote `origin/main` is fetched every three seconds plus network time.
The optional post-commit hook leaves a wake hint; polling works even without it.
Full behavior, rollback and logs: [DEPLOY.md](DEPLOY.md).

Lua/content-only commits use the existing binary and atomically switch the pack.
Rust changes reuse a Cargo build cache, health-check a candidate, then restart the
host using the existing save. Native clients reconnect with their previous identity.
The runner records measured deployment duration; a cold or large Rust build has
no seconds-only guarantee. The runner builds only the headless server feature set
to avoid graphics builds for server changes. Rebuild the normal binary on each
client for client changes. A protocol change needs a new protocol version and
compatible client builds; server deployment does not replace client executables.

```sh
python3 dev/runner.py stop
```

The runner continues after the terminal closes, until stopped, logout or reboot.
It is a local process, not an installed login service. Run `start` again after
login. There is no GitHub webhook receiver or public tunnel to configure.

## Coordinate agents

Use one worktree per stream and claim a narrow set of files before editing.
See [the coordination runbook](AGENTS-WORKFLOW.md). Local claims share a lock across
linked worktrees; separate computers/clones still need a shared ownership agreement.
Normal fast-forward pushes serialize publication, and a rejected push requires
reconciling and retesting against the new `main`.

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p living-empires --no-default-features
python3 -m unittest discover -s dev -p 'test_*.py' -v
python3 dev/smoke.py
```

The smoke test uses a disposable local Git remote and real Rust hosts to measure
content and native deploys, including failure recovery and real Lightyear clients.
It needs local TCP and UDP ports. `--target-cache /absolute/path/to/target` reuses
a warm Cargo cache; the initial timing is then explicitly labelled warm.
[Verification evidence](../docs/development-verification.md) records the measured
results and remaining limits; it does not turn the full game criteria into passes.

## Why this toolchain

[Bevy](https://bevy.org/) runs the native renderer and ECS;
[Lightyear](https://docs.rs/lightyear/0.31.0/) provides UDP Netcode connections and
ordered reliable messages. The server owns state; snapshots drive the client view.
ECS replication/prediction, visibility filtering and full gameplay remain future
work. `game/` owns this native shell; `dev/runtime/` retains the tested Lua and
persistence core plus its original HTTP test harness.

[`mlua`](https://docs.rs/mlua/0.11.6/mlua/) embeds Lua with memory/instruction limits.
[Git hooks](https://git-scm.com/docs/githooks) handle local events, while bounded
remote polling handles commits made on other machines. The small Python supervisor
adds immutable snapshots, health checks and rollback. [Watchexec](https://watchexec.github.io/)
is a good optional edit/build/restart tool; this project's commit deployment needs
those additional promotion rules. Cargo Watch is deprecated by its maintainers.
