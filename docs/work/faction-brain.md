# Faction brain — bounded F0/D0 action selection

Owner: empire-brain chat 01a17437-6306-7ac2-a731-8b43d2e66ec7.
Baseline: 347c8608d13a730d37fab3d7b82d07dfd513ae16.
Primary ACs: FAC-03, FAC-12, GEN-07 (supporting behavior, not full passes).
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
kind=unit_need or blocked_need, ACL=[own faction]. at is UNIX UTC seconds.
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
observations, unmet unit-role needs, saved role priorities, actual available metal,
spending ceiling and candidate actions supplied by authority. Reject a mismatched
actor, invalid evidence or malformed view before producing any report. A candidate
is a request for a design or a proposed paid build naming an approval fingerprint;
it never grants inventory, approval, roster access or a unit.

The default Lua policy ranks feasible candidates by need urgency times saved role
weight, then stable candidate IDs. A replacement Lua policy may select any feasible
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
