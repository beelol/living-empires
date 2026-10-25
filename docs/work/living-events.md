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
