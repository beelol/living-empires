# Living world meteor increment

Owner: living-events chat. Base `347c8608d13a730d37fab3d7b82d07dfd513ae16`.
Primary EVENT-01/02/03; affected EXP-10, FAC-02/06, WORLD-04/05/06, UX-01.
Owned paths: `dev/runtime/src/events.rs`, `dev/runtime/tests/events.rs`,
`game/src/events.rs`, `game/src/event_visual.rs`, this record.
Full canonical README read before implementation; no AC is checked off.

## Design and plan

User requested random world events, public awareness, observable empire reactions
and world changes within one hour. First event: seeded meteor warning, impact,
temporary authoritative circular passage exclusion, permanent visible scar and
restoration after cooling. Broadcasts are explicit observation receipts;
unreported hidden audit hazards cannot enter another faction's knowledge.
Presentation-only effects would fail EVENT-01; a full supply-route economy is
unavailable. This increment dispatches pre-existing **fixture aerial survey
drones**, with authoritative positions and completed orders, without granting
inventory, territory or paid units. They are not model decisions.

1. Write/run failing transition, collision, restart, privacy and replay tests.
2. Implement deterministic simulation and atomic locked sidecar persistence.
3. Add host-owned elapsed-time adapter; gate inactive deployment candidates.
4. Add native warning, scar, public bulletin and actual survey motion.
5. Hand off shared export/network/authority hooks to terrain owner; compile/test
   integration and capture actual client evidence.
6. Serialize explicit-path local main checkpoint; record activation status.

Stable authority/persistence/geometry is Rust. Current rules use fixed development
values; Lua hot policy tuning is a follow-up dependency. Existing worktree predates
coordinated direct-main instruction and is preserved until files transfer to main.

## Contracts and dependencies

Terrain chat owns runtime lib export, server, world, protocol, client and CLI.
Agreed `ClientState.events: EventSnapshot`, additive
`ServerUpdate::WorldEvents { snapshot: EventSnapshot }`, explicit `--events-demo`,
and `move_player_checked`: collision after duplicate replay resolution but before
commit. Default F0/D0 runs have no meteor timer. Terrain v1 metres x/y map to Bevy
x/z; shared terrain sample supplies height. Baseline terrain/deposits stay intact.

`EventWorld::load(path, seed)` locks and recovers the independent sidecar;
`set_center([x,y])` configures only an unstarted world from a terrain-validated
site; `advance(seconds)` persists at most 3600 simulation seconds per call.
`snapshot()` exposes explicitly public events and eligible public motion.
`observations(actor)` and `responses(actor)` are actor-scoped evidence.
`observe(actor,id)` is authority-only after sensor/report validation, not an API
for player/model proposals. `segment_blocked(from,to)` sweeps the whole segment
with one metre clearance, permits outward escape and blocks entry until cooling.

`EventHost::load(path,seed,center)`, `update(active)`, `snapshot()` keep the sole
writer on the host tick. Candidate inactivity and downtime pause time. Catchup
caps at five seconds per frame. No event thread or duplicate scheduler exists.
Native child `event_visual::install(app)` reads ClientState; empty default snapshots
remain invisible. Meshes/materials are shared or retired on phase changes.
History retains up to 256 events, then random generation stops without deletion.

Diplomacy owner referred public observation context bridging to GM owner.
GM gets source IDs and explicit recipients, never private operator snapshots.
Deterministic survey responses must never be claimed as live Argon decisions.

## Evidence

- Initial event compile failed as expected: module absent.
- Terrain-center test compile failed as expected: method absent.
- Six focused tests pass: warning/impact/cooling/restart, hidden knowledge/motion
  privacy, deterministic replay, writer/corruption, swept collision/escape and
  persisted terrain center.
- Full `cargo test -p argon-dev -- --test-threads=1`: 64 tests passed
  (28 library, 1 CLI, 13 content, 6 events, 16 host), zero failures.
- A seventh focused test passes: failed persistence leaves the authoritative
  event clock and events unchanged, retry succeeds exactly once.
- Native renderer compiled and passed clippy with warnings denied in an isolated
  engine harness using the agreed ClientState fields. Host adapter inactive-gate
  and public-feed test passed there. This is not shared-client integration proof.
- Actual Bevy fixture renders captured warning/impact/survey. First run showed
  all stages; review found a missing font glyph and flat impact shading, fixed.
  Second run warning rendered, later captures were black; retained as failures.
  Combined hooks and integrated multiplayer evidence remain pending terrain owner.

## Acceptance limits

EVENT-01/02/03 remain blocked as full-game audit results: physical supply convoys,
dependent production, alternate resource routes and five live-agent runs are
unavailable. Two actual client views and authoritative collision still need
verification after hooks land. No LIVE release or full-game pass is implied.
D0 generated-unit approval/paid deployment belongs to other owners; fixture drones
do not satisfy D0.

## Resumed goal and durable broadcast checkpoint

User resumed work with an explicit active goal and asked to push every working
checkpoint normally to main. Existing isolated workspace refreshed to e99e70c
after preserving identical published source copies. Same event ownership, plus
new `game/tests/events_network.rs` for the agreed real Lightyear contract.

GM owner accepted public Event v1 envelopes: `{id,source,at,kind,payload,
allowed_recipients}`. `EventWorld::advance_recorded(seconds,receipt_unix)` now
commits each newly observed public phase and its outbox record in the SAME save
transaction. IDs contain a generated-once persisted world namespace, meteor ID
and phase. `at` is captured UNIX receipt time; payload `simulation_time` and the
event's warning/impact/cooling times remain simulation time. A crash/retry cannot
manufacture a second broadcast or replace its original receipt. Private audit
hazards never enter this outbox. Old sidecars load with default empty outbox and
receive a namespace without invented historical timestamps.

`broadcasts()` and `EventHost::broadcast_feed()` supply this durable source to the
existing sole GM scheduler. The frequent public client snapshot remains unchanged
to avoid retransmitting a growing history on every gameplay update. No provider
or scheduler is started by this module. Fixture-only `advance()` records no fake
wall clock. Host `update()` uses captured real time for recorded advancement.

Validation: 11 focused event tests pass, including deserialize into the actual
GM v1 type, duplicate ingest without a revision change, exact-payload routing,
atomic phase/outbox failure recovery, backwards receipt clocks and legacy loads.
Native adapter test and standalone native clippy pass. Restricted full runtime run
failed nine network fixtures on socket permissions; approved rerun passed all34
library cases and CLI/construction, then retained the existing one-second
`native_collection_operations_cannot_bypass_instruction_limits` timing failure.
That case passed isolated retry (0.91s); no guard or unrelated content code changed.

Actual 1280x800 Bevy render-target captures now show all three fixture stages;
manifest `/private/tmp/argon-meteor-render-manifest.json` binds source and image
SHA256, UNIX capture times, seed77 and ticks35/45/75. Earlier black window captures
remain failed evidence. Neither rendering method is shared-client proof.

The new real two-client event test compiles but fails as expected on the current
host's unsupported `--events-demo`; it remains unpublished until shared hooks
land. Terrain757ee07 handed shared authority/client files to the construction
authority owner (thread01a171d8). That owner was asked for a bounded event-hook
handoff after the current city integration. No shared file is edited here.
