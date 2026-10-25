# Paid construction authority

Owner: Build fast iteration workflow, thread 01a171d8-149a-7613-bd7d-1eb488e82b22.
Claim: bilal-build-authority. Primary SET-04, GEN-07; affected PLY-11,
WORLD-02, WORLD-05. Full README read. No full-game AC passes claimed.

## Design and bounded checkpoint

Implement a deterministic construction state transition module, consumed later by
the existing authoritative World transaction. Never create another save writer or
network command handler. The caller supplies authenticated actor, server-approved
recipe and placement eligibility; neither a client price nor an AI proposal grants
authority. Cost and exact approved content fingerprint are frozen in the accepted
order. Progress uses explicit simulation ticks, pauses without builder/power,
and completes once. Cancellation refunds only unconsumed reserved metal, once.
All public mutation rejects invalid input without changing state.

Use integer metal and work ticks, with consumption rounded up proportionally to
completed work. Deduplication is by actor plus request ID and complete immutable
request; reusing an ID for a different request rejects. The module is not itself
a durable acknowledgement: World must atomically save it with inventory before
acknowledging. A development fixture recipe cannot establish LIVE approval.

Alternatives: a separate construction save would split inventory transactions and
risk duplicate payment; rewriting World now would collide with terrain. A pure
state transition module provides a small testable contract without either risk.

## Ownership and dependencies

- Owned: dev/runtime/src/construction.rs, dev/runtime/tests/construction.rs, this file.
- Terrain chat owns World, protocol and client. Its metre coordinates, terrain v1
  sample/traversal and saved-world migration must precede placement integration.
- Existing sole diplomacy scheduler owns faction proposals. No new scheduler.
- Studio owns reviewed bytes/provenance; final approval interface remains unsettled.
- Runtime export/server wiring requires handoff. Until then tests include the
  standalone module directly; no claim that player controls can construct yet.
- UI owner supplies construction controls and actual two-view evidence later.

## Plan and checks

1. Tests: reserve once; changed retry rejected; insufficient funds unchanged;
   wrong owner rejected; pauses; partial cancellation exact; completion no refund;
   JSON roundtrip preserves deduplication and frozen costs/content hash.
2. Implement minimal serializable integer transition state, no I/O or new crates.
3. Run targeted tests, rustfmt, clippy; inspect current main and ownership before
   integration; run combined project checks and push normal fast-forward checkpoint.
4. Arrange World/Studio contract handoff, integrate durable actions and client
   controls, then verify actual multiplayer restart and approval paths.

Checkpoint: deterministic accounting implemented; four construction tests and
targeted clippy passed. Full workspace suite at foundation baseline passed 44
tests with local process/network permission. The first restricted run failed the
three existing one-second content subprocess guards; the permitted rerun passed
all three without changes. Shared-main combined verification remains pending.
Exact candidate at base 347c860 passed all 73 Rust tests using this stream's
separate build cache. Earlier main/shared-cache runs retained content subprocess
timeouts and one unfinished faction-brain compile failure; no other stream's files
were changed. Shared build-cache results are not accepted as exact-tree evidence.
Upstream 98e5564/a8984a6 add Lua filename validation and Studio tooling; final
integration reruns affected content/accounting tests against that baseline.
No actual approved content, placement, durable world transaction, or multiplayer
construction has been exercised by this isolated module.
