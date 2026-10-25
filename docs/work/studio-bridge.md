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

First checkpoint published: a8984a631a52e54dac8e50812218748084d2ca32,
normal FF push to living-empires origin/main from fetched 98e5564. Remote ref
verified. Hourly thread heartbeat integrate-pulkit-studio-with-living-empires
created ACTIVE and viewed in the app. Goal remains active until live integration.

Next increment: receiver-side archive verifier checks producer SHA256, full
history chain/state hashes, derived diffs, exact image bytes/time/content and
manifest membership without extraction, database creation or asset activation.
21 fixture tests now pass. Independent review found receiver metadata values
were less strict than the producer; a failing regression reproduced this and
both paths now share the exact same provenance validator. Pulkit location/interface requested after local
handoff searches found no endpoint/contract; actual Studio integration pending.

GM owner subsequently settled `<world-state>.gm-inbox.json` array intake and
`<world-state>.game-master.json` sidecar. Added atomic scoped union publication
to that exact intake contract; no duplicate scheduler or world writer. 23 tests
pass including actual CLI publication, ACL preservation and protected source DB.
Host wiring remains the GM owner's next step, not a live result here.

User suggested Studio should be on main. Read-only GitHub live tree inspection
at a8984a6 and then 05cf47d (complete, not truncated) found only this dev/studio/
bridge, no Studio service/transport; available branch list was main/master.
Older argon-empires repository lookup returned404, so its contents are unknown.
Coordinator has no newer Pulkit path/commit; do not assume an endpoint.
Receiver checkpoint waits in agreed integration queue behind existing owners.

GM importer limits subsequently agreed: 1MiB/1024 events per array, 16KiB payload,
64 unique recipients, 160 ASCII identifier chars from alnum/-_.:/. Producer now
enforces these and exposes receipt-driven paging (`last_id`, `has_more`,
`--after-event`) so long histories can continue without truncation. Cursor
advancement is the existing consumer's responsibility after durable import,
never a producer assumption. 25 tests pass including exact event-count and byte
boundaries. Receiver and atomic publication were independently reviewed; no
outstanding important findings after the provenance fix.

Further owner clarification: GM receipts are durable sidecar events[].id and
future operator status game_master.events[].id. Its demo ledger currently stops
at 1024 unique events TOTAL, not merely per batch. Paging does not bypass that
limit. This is an explicit remaining dependency for sustained continuous worlds;
no automatic eviction/rotation or pretend unlimited integration. Current D0
bounded proof can use remaining capacity; final continuous goal stays incomplete.

Cross-language fixture verification: copied the GM owner's exact module bytes
(SHA256 59e1d041e37124048a3348fc8c3710a38e8908ef9d982b1e6bb32d9e497e17aa)
into an isolated offline Rust harness at /tmp/argon-studio-gm-contract. The real
Python publisher wrote two eligible events while excluding unknown-faction and
operator-only facts. The real Rust import_file accepted them, repeated import
did not change state, permitted Helios routing succeeded, Cinder routing failed,
and reopening the durable GM ledger retained two event IDs and scoped delivery.
Harness exited0 with all assertions passing. This certifies the file interface,
not live Argon calls, accepted world effects, actual captures or D0.

Latest combined Python check used immutable snapshot faa17d7 plus this owned
candidate at /tmp/argon-studio-check-gh_zy3xw. 39 tests ran: 36 passed, two errors
(test_failed_candidate_probe_never_stops_live_host,
test_host_preserves_explicit_env_file: localhost connection refused following
three-second fixture validation timeout) and one failure
(test_failed_live_start_rolls_back_previous_binary_and_content: fixture hit
validation timeout before the expected injected live-start error). Earlier
38-test baseline passed; preserve both outcomes. Studio's 25 tests pass.
Runner owner informed; this stream does not change its deadlines/implementation.

User clarified Pulkit Studio is not pushed yet and asks readiness for when it
lands. Keep existing hourly heartbeat and live dependencies; do not invent the
missing provider or claim end-to-end integration before that push.
