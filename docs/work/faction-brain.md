# Faction brain — bounded F0/D0 action selection

Owner: empire-brain chat 01a17437-6306-7ac2-a731-8b43d2e66ec7.
Baseline: 347c8608d13a730d37fab3d7b82d07dfd513ae16.
Primary ACs: FAC-03, FAC-12, GEN-07; city increment supports SET-01/02
(supporting behavior, not full passes).
Affected ACs: FAC-02, FAC-05, FAC-06, FAC-09, LIVE-01, UX-04, AUD-04,
SET-04, WORLD-02, WORLD-05.

## Ownership and contracts

Exact owned paths: dev/runtime/src/faction_brain/, dev/runtime/tests/faction_brain.rs,
docs/work/faction-brain.md, docs/superpowers/plans/2026-10-25-faction-brain.md.
The main checkout has the coordinator's documentation claim. The coordinator
explicitly permits per-stream disjoint ownership records because dev/agent.py
supports only one claim per checkout. No other claim is overwritten or released.
All staging/commit/push operations use the integration lock and exact owned paths.
User requests continued work and working checkpoint pushes to main.

GM owner: Connect agentic layer to Argon GM / bilal-argon-gm. Agreed v1 report:
{id, source, at, kind, payload, allowed_recipients}. source=faction-brain,
kind=unit_need or blocked_need, plus agreed building_need/blocked_building_need
for the cities-first increment. ACL=[own faction]. at is UNIX UTC seconds.
An ID identifies immutable facts, source, time and ACL; the GM rejects conflicting
reuse. GM owns durable intake/deduplication, private memory, prioritization and
pending request_content routing to Studio. This module owns no history writer.

Diplomacy owner: Connect Gemini diplomacy loop. Preserve the sole scheduler,
Gemini provider and seven-field diplomatic proposal; no extra loop or live calls.
The existing diplomacy context has no economic inputs. Future host adapter work
requires that owner's explicit handoff.

G/build-authority owns paid construction and validation; A/Studio owns generation,
actual Bevy renders, independent checks, approval of exact bytes and release.
Terrain owns lib.rs/world.rs/server/protocol integration. No central export edit
is included. The integration test loads this module by path until that owner
exports it. No authoritative gameplay schema is changed.

## Small design

Use a faction-bound planner from an authoritative faction view: explicit allowed
observations, unmet unit/building-role needs, saved role priorities, actual available metal,
spending ceiling and candidate actions supplied by authority. Reject a mismatched
actor, invalid evidence or malformed view before producing any report. A candidate
is a request for a design or a proposed paid build naming an approval fingerprint;
it never grants inventory, approval, roster access or a unit.

The default Lua policy prefers feasible building needs, then ranks within each
category by need urgency times saved role weight and stable candidate IDs.
A replacement Lua policy may select any feasible
candidate; it cannot widen feasibility. Rust computes the exact integer score and
passes a sortable string to avoid Lua overflow/rounding. No world state enters Lua.
An explicit blocker or unaffordable build cannot be selected. If an unmet enabled
need has no feasible candidate, return blocked_need; a resolved/disabled set is
idle. Swapping only weights must swap an otherwise equal defense/trade choice.
Return one private GM report linked only to the selected need's observation IDs,
revision, proposed action and constraints. The authoritative executor must recheck
current revision, rights, costs, approval bytes, consent and idempotency at commit.

Managed hosting was considered; it is unnecessary for this bounded policy and
would duplicate orchestration already owned elsewhere. Existing Argon decisions
remain the live model path. This deterministic policy is preparation for that
path, never evidence of a live model decision.

## Delivery and checks

First checkpoint: tested selection/report module with a Lua policy. No scheduler, Bevy view,
live need source or actual construction integration is claimed by this checkpoint.
Tests cover independent faction weights, visibility, missing evidence, explicit
blocks, insufficient funds/ceiling, malformed approvals, resolved needs, stable
ties and exact private report fields. Run focused Rust tests, workspace tests
serially, targeted formatting and clippy. Preserve failures and report integration
and activation separately. README criteria stay unchecked.

Progress: first behavioral red was 8 failing selection tests/4 passing rejection
tests. Initial implementation passed all 15 focused tests and targeted clippy.
The initial restricted workspace run failed eight existing Gemini local-listener
tests with OS permission errors; permitted serial rerun passed. Independent review
found cross-faction policy promotion state in a shared Planner. A new regression
failed, then Planner was bound to one faction at construction; each choice also
uses a fresh sandbox VM. Selected actions now carry actor and observed revision
even when separated from their report. Final combined verification follows.

Lua policy API: Planner::new(faction_id), stage_policy(source), then plan at the
next host decision boundary. A syntax-invalid stage leaves the active policy;
an invalid, blocked, looping or unavailable choice falls back to last good. A
previously promoted policy that fails on new input falls back to packaged rules.
Every decision/report carries the exact policy SHA256 and rejection status.
The host must read changed source and call stage_policy; no file watcher or host
connection is included yet. Editing policy.lua alone does not update a running
binary; this is an explicit activation dependency, not live hot-reload evidence.

Verification checkpoint: committed-main a8984a6 plus exactly this stream's five
files passed all 87 Rust workspace tests serially in a temporary source snapshot
and separate cloned build cache. Focused suite is now 16/16; targeted clippy and
rustfmt pass. The later shared-main run had two preexisting one-second content
subprocess guard failures (native_collection_operations_cannot_bypass_instruction_limits
and native_pattern_work_cannot_stall_loading_or_movement); no guards were changed.
On refreshed 05cf47d, both those guards and later
special_files_cannot_block_content_loading failed in combined runs, then passed
separately unchanged. All 16 planner tests passed again against 05cf47d. These
split rechecks do not turn the failed full-suite attempts into green runs.
Snapshot build emitted the existing macOS large unwind-table linker warning.
No live policy call, real need, unit deployment, activation or Bevy view evidence
exists for this checkpoint. Integration must recheck later main drift.

Integration base: d9b40056b90a61483f01703d90b986763de929ec, equal to fetched
origin/main. Inspected intervening terrain/core export, construction and GM inbox
commits; none changes this module's input or report contract. On that combined
tree, all 16 focused tests and targeted clippy passed again under the integration
lock. Other Studio edits remain owned by their stream and are not staged here.

Ruling: user said continue after the recommended approach; proceed with the small
game-managed policy, without another platform-choice approval loop.
Ruling: changeable selection lives in Lua; Rust keeps scoped validation and exact
integer score encoding. This follows the coordinator's updated live/Lua direction.

## Cities-first follow-up

User's latest priority is three starter civilizations (organic aliens, cyborg
aliens and robots) in the same live world, with coherent cities grounded on terrain
and connected usable navigable space. The planner's next supporting change adds NeedCategory
(unit/building), defaulting missing v1 JSON categories to unit, and a BuildStructure
proposal carrying design_id, exact approval reference, paid cost and an opaque
site_id supplied by authority. Existing Rust callers constructing Need literals
must supply category; repository search found no consumers outside this module's
tests. No diplomacy schema, saved world or GM envelope changes.

GM explicitly agreed building_need/blocked_building_need and will request content
only for actual unmet building needs. Terrain explicitly states that
Terrain::sample and can_traverse([f64;2],[f64;2]) do not establish building rights,
footprint/entrance clearance, site approval, ownership or mining rights. G/terrain
must supply those contracts and real legal candidates. Actual city placement,
coherent layout and navigation proof remain BLOCKED. Site IDs in tests are
fixtures, never fabricated live approval tokens. No site IDs are generated here.

The same Rust validation enforces positive paid costs, exact hash syntax, budget
and authoritative blocker codes for both unit and building proposals. A unit
recipe cannot fulfill a building need or vice versa. Lua now gets the explicit
category and prefers feasible building work before other empire work; weights
still select among needs in that category. An unknown building may produce only
a design request; missing legal actions remain blocked. No build completes here.

City TDD: after type scaffolding, four new behavior tests failed (report kind,
affordability, category/site validation and design-request kind); the implementation
made them pass. A separate city-first Lua test then failed against the old weighted
default before the policy change. Legacy category-free unit JSON remains covered.

City verification: frozen committed-main cebe62f05b9fcef0e253b33d8d43f25bb6f7c45e
plus this stream's five files passed all 116 Rust workspace tests serially in the
separate cache, including the three content startup guards that failed in earlier
attempts. Those earlier failures remain recorded above. All 22 focused tests,
targeted clippy, rustfmt and diff checks pass. Independent city-delta review found
no actionable issues. Actual live city placement/connected navigation is still
blocked; none of these fixture tests establishes that proof.

After the public GM/planner exports landed, a real file-backed interoperability
check was added: deserialize the building report as GM Event, ingest twice once,
reject routing to Tide, route to Helios, restart and recover that exact report.
The focused suite is now 23/23 and targeted clippy passes. Production source is
unchanged from the 116-test frozen run; the added consumer check uses the committed
GM public interface. No live model or World placement is exercised by it.

G/build-authority confirmed Construction::start consumes a trusted server-frozen
Recipe but does not approve roster, cost, site or actor identity by itself. There
is still no live World adapter or agreed faction-local inventory/site identity.
BuildStructure execution stays blocked until the actual World owner supplies it.

Proposed minimal LegalSite v1 split sent to G/terrain/GM: G owns a faction,
world/terrain revision and exact recipe-bound site lookup; terrain supplies actual
ground pose, footprint/entrance validation and a concrete connected route from an
owned anchor; G validates rights, overlap and payment in the single World
transaction. This is not a settled implemented interface yet. No more standalone
planner expansion is planned before one real grounded paid building and route
proof. Active-path Lua watcher -> validation -> decision/tick activation -> client
visibility is also not wired by this module; source staging API tests do not
establish that live path.

City integration base: c1f5161b119553e582f3b619a0bed45c9c8e579b, matching fetched
origin/main. Inspected intervening event modules, public core exports, Studio
receiver, native launcher and runner recovery; no conflicting planner contract.
Final verification caught a shared-cache run executing only the old 16-test
binary despite 23 tests in source; that run was discarded as candidate evidence.
Rebuilding in this stream's private cache compiled the actual current source and
passed all 23 tests, including the real GM consumer check, plus targeted clippy.

## Live adapter checkpoint (contract v2)

The README was revised on main at 99b9fa8: three empires, camera only, cities and
usable generated buildings/units. Primary FAC-03/FAC-12/GEN-07 and affected
FAC-02/FAC-06/FAC-09/LIVE-01/UX-04/WORLD-05 remain supporting work, not passes.
Owned paths remain this module subtree, its test and these two stream documents.
G owns atomic publication of actual private FactionView v1 inputs; GM owns polling
on the existing worker and durable ingest acknowledgements. No second scheduler
or World writer is authorized. The producer and hook remain pending handoffs.

Pending safety fix: disallow Lua metatable registration because __gc finalizers
can run during VM teardown with instruction hooks disabled. The regression first
reproduced a three-second child-process hang; the guarded sandbox passes. Its
last-good assertion now first promotes a custom hauler policy, so resetting to the
packaged default cannot falsely pass. Final fresh verification follows.

Adapter correction agreed with G: all durable outboxes remain World-owned. This
module adds a read-only LivePlanner instead of a second retry-state writer.
Proposed input is {schema_version:1, observed_at:<persisted source UTC seconds>,
views:[FactionView v1]}, published atomically from actual authority. Poll time
cannot replace observed_at. poll(path, now) rejects future observations, duplicate
factions, private evidence, unsupported schemas and oversized/nonregular files.
At most three views and 2 MiB are read; the optional explicit policy path is
limited to 64 KiB. No ambient repository watcher or model call is added.

LivePlanner::new(Option<PathBuf>) -> Self; poll(&Path,u64) -> Result<Vec<NeedReport>>;
policy_error() -> Option<&str>. Each faction keeps its own last-good runtime Lua
planner. Event IDs hash the entire immutable envelope with a fixed placeholder ID,
including original source time, policy hash/rejection and private ACL. Repeated
polls intentionally return the same immutable envelope for durable GM dedup.
Callers must retain full reports until acknowledgement; G owns that persistence.
Missing source is empty, never fabricated needs. Syntax-invalid policy retains
last good and exposes a separate diagnostic; no world state is mutated.

Initial adapter red: unresolved LivePlanner import. Implementation green: all 27
focused checks, including real GM ingest dedup across repoll/restart. Added a
per-faction choice/privacy batch and duplicate-view check. Clippy caught one
needless test borrow, corrected before final verification. Producer observed_at
contract and worker integration are still awaiting explicit owner confirmation;
these tests use labeled fixtures and establish no live city/Foundry or D0 pass.

G and GM both explicitly confirmed the final observed_at v1 contract. G retains
actual filtered producer/outbox ownership; GM retains its sole worker hook. G's
urgent hot-Lua checkpoint 8aceb19 is published; canonical city promotion has
priority before this adapter's integration slot. Fresh adapter verification is
28/28 focused tests plus targeted clippy -D warnings; diff/rustfmt checks pass.
A same-packet new-instance test compares the complete reports, not merely IDs.
Independent review is in progress. Actual live source/hook/delivery still pending.

Independent review found no actionable blocker under the agreed durable-outbox
contract. Explicit limitation: restart-stable reports require unchanged selected
policy/report facts. A custom in-memory last-good policy does not survive restart
if its source is missing or malformed; that case can select packaged rules and a
different report ID. The owning caller must retry its retained full envelope,
not recompute an acknowledged/pending action from a new policy. Actual World
execution still revalidates revision, rights, resources and idempotency.
Frozen a93f8d6 plus only the five owned files passes 28/28 tests and targeted
clippy -D warnings. Later 8894494 drift touches the separate asset lane only.

Integration checkpoint: fetched origin/main c0b802d95f4e5e614e3d42a6e09ca31778edb060
under the integration lock. Intervening asset-lane changes do not alter faction
inputs, GM schema or these owned files. Froze that exact committed base plus only
the five owned files: all 28 focused tests and clippy -D warnings pass again,
with exit zero. Formatting and diff checks pass. Foreign dirty files remain
unstaged. No solo-colony work is included. Publication follows normal main push.
