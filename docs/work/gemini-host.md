# Gemini diplomacy demo — host integration

Owner: Bilal / root host stream. Baseline: db64e4d.
Primary: FAC-02, FAC-09, DIP-05, DIP-14. Affected: FAC-01, FAC-06,
DIP-03, DIP-04, DIP-13, WORLD-05, AUD-04, UX-04.

The user requested a small continuously deciding diplomacy demo, reading the
Google Doc, with the Rust app loading `.env.local`; use Flash until Argon is
available. The user subsequently supplied the intended key and explicitly chose
Argon again; `gemini-4-argon` is the final default and local setting. Both tabs of
the linked Google Doc were read through Chrome.
README is canonical where the older document differs about checklist location.

Plan: (1) keep credentials outside Git and load host config dynamically;
(2) run one asynchronous model request at a time, rotate three fixture factions,
and apply validated proposals to a separately persisted diplomacy ledger;
(3) show the loop in a localhost-only operator page; (4) verify consent,
visibility, persistence, failure recovery and actual model calls, then integrate.
Native movement/protocol and the commander save schema remain version 1.
Diplomatic pacts are demo ledger records; combat, goods, physical trade and
generated leader art are unavailable, so this is not a full-game AC pass.

Owned paths: .gitignore, .env.example, dev/runtime/Cargo.toml, Cargo.lock,
dev/runtime/src/lib.rs, dev/runtime/src/diplomacy_loop.rs, game/src/admin.rs,
game/src/server.rs, game/src/diplomacy.html, dev/runner.py, dev/test_runner.py,
dev/README.md, this record. Claims recorded by dev/agent.py.

Dependencies: diplomacy-core owns `Diplomacy` v1 (load/context/snapshot/apply,
record_attempt/record_failure/attempts/revision and FACTION_IDS); gemini-transport
owns `GeminiConfig`, load_config(Path), decide(config, context). Parent owns
exports and dependency versions. These interfaces were agreed before editing.
Attempts do not advance the gameplay revision; accepted diplomatic effects do.
The operator view is loopback-only and is never replicated to player clients.

Checks: focused Rust state/provider/loop tests, real HTTP operator and native
movement test, Rust fmt/clippy/workspace suite, Python runner suite, bounded live
Flash run with input/proposal/outcome records. Failed outcomes retained.

Progress: isolated worktrees and claims established; Google model list confirms
gemini-3.8-flash for this key. `.env.local` also protected immediately by local
Git exclude without reading or rewriting its key.

Integration evidence (October 25):
- Core and transport commits handed off to this stream and incorporated;
  all owned paths now covered by the parent claim.
- Live Rust host on localhost:4317 used the actual `.env.local` with Argon:
  six attempts, six accepted outcomes, zero provider failures, one mutually
  confirmed pact. Sequence: Helios offer, Tide acceptance, Cinder message,
  Helios message, Tide stance, Cinder message. Each faction took two turns.
  Full inputs/proposals/outcomes retained in the ignored local evidence world
  `.local/diplomacy-argon-check/world.json.diplomacy.json` in the primary checkout.
- Actual browser view inspected; all three factions, model, call budget,
  accepted pact and recent decisions were visible. Native Lightyear probe
  accepted two movement actions while this host and diplomacy worker ran.
- Review found interrupted-attempt recovery and retry-interval issues. Both
  reproduced with failing tests and fixed; the reviewer rechecked both fixes.
- Earlier Flash connectivity attempts returned two HTTP503s, then succeeded;
  Gemini2.5 returned404 for this account. These are retained as failed provider
  checks, not successful autonomous runs. Final six-turn check used Argon only.
- Initial parallel verification hit existing timing guards in three content
  tests and startup timeouts in three Python deployment tests. Subsequent
  serial/full reruns are recorded below; no timing guard was loosened.

Terrain handoff: after this commit is integrated and `bilal-gemini-host` is
released, the terrain stream may claim `dev/runtime/src/lib.rs` and
`game/src/server.rs`. Preserve `DiplomacyLoop::start(state_path, env_file,
activation_file)` and its independent worker/status handle. No native network
schema or commander save schema was changed by diplomacy. Its separate ledger
has version1; do not merge private operator snapshots into player replication.
The host starts this worker only after UDP startup, and preflight candidates
cannot spend before activation. The deployment runner passes ARGON_ENV_FILE
from the primary checkout; keys never enter immutable release snapshots.

Empire-agent coordination: an independent read-only AC-gap/adapter assessment
is complementary. This stream owns the current fixture faction identities,
allowed observations, strict proposal schema and sole diplomacy writer. A future
brain adapter should consume `Diplomacy::context(actor, now)` and return the same
seven-field proposal through the existing validator, without reading the saved
ledger directly or starting a second decision loop. Broader economy/orders and
generated faction identity integration need a separate agreed contract; they are
not supplied by this demo. Keep these files unchanged until ownership is handed
off after integration.

Final verification: 69 Rust workspace tests passed with one test thread;
formatting and workspace clippy passed. The real Rust smoke test passed all eight
checks: two Lightyear sessions, Lua-driven movement, dirty-file isolation, Lua
deploy, invalid Lua rollback, repaired release, native restart persistence,
and remote deployment without checkout mutation.
Python full runs retained startup-timeout failures; targeted retries verified
the affected behaviors without changing their existing timeouts. Credential
scan found no API key in candidate tracked or untracked files.
