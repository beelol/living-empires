# Studio bridge work record

Owner: bilal-studio-bridge (this chat). Primary AUD-03, AUD-04, UX-04,
LIVE-07; affected GEN-07, FAC-02, FAC-05, LIVE-01, WORLD-05.

Owned paths: dev/studio/, this file, and the 2026-10-25-studio-bridge
spec/plan. Explicit disjoint main-checkout ownership granted by milestone
coordinator 01a1743c-661f-76e0-bb0a-519ae7fb801c on 2026-10-25.
Its registry claim remains its own; one claim per checkout prevents adding a
second claim. Use serialized integration lock and exact staging paths.

Dependencies: GM owner 01a1743c-f377-7c22-9daf-997fa34471d5 agrees v1
Event {id,source,at:u64 UNIX seconds,kind,payload,allowed_recipients}.
Empty recipients means operator-only, no wildcard. GM owns consuming/wiring.
Diplomacy owner 01a1741b-5050-7891-9c42-eaeb2d55adee owns sole decision
scheduler; no duplicate scheduler here. Pulkit owns Studio and its unpublished
transport; use file export until its contract exists. Game authority owners
own recording accepted actions, release approval/activation and paid builds.

Scope: minimum durable replay/evidence and studio export adapter for D0.
Hash-linked event IDs, immutable render blobs, pinned release/rules/assets/seed,
recorded state changes, deterministic reducer verification, recipient filtering,
and restart-safe content notices. No generated proposal activates content.
No blanket feed to all minds: each gets only allowed observations/capabilities.

Checks planned: corruption, reorder, truncation, duplicate conflict, concurrent
writers, restart, failed write, evidence mutation, hidden event privacy,
duplicate activation and stale decision acknowledgments; existing project tests.
Fixtures cannot establish full-game AC pass or actual Bevy render evidence.

## Checkpoint evidence

Implemented stdlib SQLite journal (FULL synchronous transactions), canonical
SHA256 linked events/state snapshots, exact immutable image blobs/provenance,
operator ZIP archives with readable diffs, deterministic pinned-rules replay
verification, scoped GM v1 JSON export, content activation notice dedup and
durable decision acknowledgments. CLI append accepts authority outbox files
outside the tick; no network service, model call or new scheduler.

18/18 bridge tests pass (2026-10-25). Tests were written before implementation;
initial missing module and feature failures retained in chat. Independent reviewer
found activation retry conflicts, Python bool/int equality and existing file
permissions gaps. Regression tests failed first, fixes pass all 18.

Project Python suite: first restricted run 11 passed/27 localhost permission
errors; permitted rerun all 38 passed. Rust first restricted run 20 passed/8
localhost errors. Permitted run: 28 library + 1 CLI + 4 construction tests passed;
content tests 10 passed/3 failed one-second subprocess guards:
native_collection_operations_cannot_bypass_instruction_limits,
native_pattern_work_cannot_stall_loading_or_movement,
special_files_cannot_block_content_loading. A follow-up workspace run stopped on
the other stream's in-progress untracked faction_brain::Planner import. Those
files were preserved and owner/coordinator informed. Do not claim workspace green.

Ruling: notice-set freshness is checked here; decision-state fingerprint is
caller-attested provenance. Live authority must revalidate current state before
effects, since this adapter cannot inspect live units/resources/rights.

GM explicitly owns consumer wiring and agreed to union known-faction exports,
dedup event IDs and preserve ACLs. Diplomacy owner defers future adapters to GM
and paid-build owner, avoiding a duplicate scheduler. Build owner confirms hash
syntax alone is never approval; final Studio approval/roster interface unsettled.

Blocked live dependencies: authoritative transaction outbox; actual Bevy capture
hook and exact independent review receipts; Pulkit transport/approval contract;
real activation -> eligible faction replan -> paid useful unit in both clients.
No actual game images were captured for this adapter and no AC is checked off.
Integration revision and monitor will be reported after publication.
