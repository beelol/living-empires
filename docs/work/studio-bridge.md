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

## Landed Foundry integration (resumed)

Foundry landed at b47a215. User now explicitly requests immediate automatic
per-empire unit/building jobs and expansion using new content. Existing owned
dev/studio/ lane covers the new HTTP adapter/controller and tests; no shared
game/server/pipeline files will be changed without handoff. Primary LIVE-01,
LIVE-03, LIVE-07, GEN-07, FAC-05; affected privacy/durability ACs above.

Actual API: POST /api/jobs {themeId/theme,provider,prompt,injectDefect}; GET
/api/jobs/{id}, /api/jobs/{id}/package and exact /evidence/{job}/{attempt}-{view}.png.
There is no server idempotency key. Persist dispatch first, embed immutable need
marker, reconcile unknown outcomes through exact GET /api/jobs matches; never
blindly retry a possibly paid creation. Studio preview activation is not game
activation. Rust world-check op=package revalidates frozen pipeline and package.

GM owner settles source decisions[] accepted request_content target=studio,
cited events[] with recipient ACLs and matching attempts[].at. Build prompts only
from cited permitted facts, not the operator/model summary. Faction-brain need
payload includes faction_id, category(unit/building), role, need_id and revision.
G/build owner owns roster, costs, paid build/recruit, persistence and live effects;
brain owner owns need production/replanning; GM owns sole decision scheduler.

Implement durable single-worker transport with per-request mapping, reservation,
unknown-outcome blocking, bounded polling, package/image verification and immutable
authority delivery files. Automatic fixture rehearsal uses zero paid calls.
Live dispatch needs explicit configured global/job allowance, never an unbounded
implicit default. Current Foundry supports only sensor buildings; genuine unit
generation/function needs an explicitly compatible producer+runtime extension.
Do not relabel an outpost as a working unit or count Three.js images as Bevy proof.

Terrain owner 01a17427 explicitly grants this stream new
game/examples/foundry_audit.rs native audit lane, no existing engine/protocol or
Cargo files. Bounded Luna worker native_foundry_audit implements it; parent owns
integration. Mapping: world x/y horizontal -> Bevy x/z; elevation -> +Y;
Foundry +Y up/+Z front compatible, any placement rotation recorded explicitly.
G owns actual paid placement/activation; captured native frames alone grant none.
User explicitly authorizes sponsored-credit spending and overrides further
spending-ceiling questions. Preserve existing per-job 3attempt/6call/20min bounds.

Actual integration evidence: installed exact locked Node dependencies, built
living-world-core with live-provider (--locked after generated lock handoff),
and started real Foundry on127.0.0.1:4317. Rehearsal job
job-argonaut-1792957123251 used realHTTP/Rust/browser, injected blocked entrance,
repaired on attempt2 and exported three actual PNG hashes. Typed Rust revalidation
caught Node's harmless12.0->12 JSON formatting; now HTTP object is reparsed by
the same Rust pipeline hash validator, with booleans kept distinct.
Immutable delivery /tmp/argon-foundry-real-api-test/ready/need-479899c7513c1f27c802fdb1d7b230a68ab289249e0315e32fb44d693091c60a
contains7files and explicit rehearsal=true/pending_authority_checks.

Native actualGPU v3 capture exited0 and produced3PNGs+completionmanifest:
/tmp/argon-foundry-native-views-v3/verified-capture-manifest.json, exact content
63052bb7371a17a236f4d4cbe7c1eab7612b7960342bc6395b3d377b4133e409,
package bbbe7c6eedca9067148899494f4c2f2db1d181bc25edf07a32b5e7260ed3b6cd.
v1/v2 failed runs retained: runner World cleared after AppExit, then unordered
capture observers saw third file before save completed. Fixed with retained
outcome and one synchronous save+completion observer; no failure was called pass.
Native cylinder is exact top/bottom-radius12segment frustum, cone6segments,
matching scene.mjs, not a max-radius substitution. Focused geometry test passes.

36 Python bridge/controller tests pass, Node23 and coreRust21 tests pass.
Independent code review provider-restart mismatch fixed RED/GREEN; durable cache
recovers post-publication crashes with stable notice bytes and directory syncs.
Initial liveworker launch was rejected by automatic review for potentially
exporting private facts. A materially safer allowlist now exports ONLYfictional
role/category/theme and opaque marker; canary tests prove no inventories, orders,
diplomacy, faction/need/source IDs or model summaries leave. Revised launch was
approved; actual continuous managed worker toolsession91515 reads real production
.local-deploy/world.json.game-master.json every5seconds. At check127GM events,
1decision,0faction-brain needs =>correctly0jobs; no production fixture injection.
G and brain owners must supply realWorldneed reports/paid expansion. Unit producer
stillunsupported, retained blocked. Full live loop remains incomplete.

Independent native-image reviewer inspectedall3 actual v3 views: silhouette,
materialcontrast, groundcontact, visibleentrance, noerror-textures/no-detachedparts
pass; detailedmaterialfinish fails (uniform surfaces, no promised abrasion/chips),
rearcanopyseam blocked. Functionaltraversal/sensors/multiplayer blocked, notpassed.
Structuredrecord /tmp/argon-foundry-native-views-v3/independent-review.json binds
exactpackage/content/3imageSHA and release_approved=false. This is useful retained
failure feedback, never silently promoted to goodquality/live activation.

Native capture integration lane: this stream names itself serialized owner of
server.mjs plus new lib/native-capture.mjs and test/native-capture.test.mjs.
Registry inspection found no matching claim; coordinator/brain/G notified of
the compatible opt-in AUDIT_RENDERER=bevy boundary. Existing renderer default,
HTTP/package schema and first-party acceptance/call/attempt/time limits remain.
Actual native images must enter the SAME review/repair cycle before approval,
not a new post-approval job that resets limits. Browser movement remains an
explicitly separate functional harness, not invented native traversal evidence.

Sponsored-credit follow-up: user explicitly rejects dollar-ceiling gating.
Named narrow serialized ownership adds lib/spend-ledger.mjs and new
test/spend-ledger.test.mjs, coordinator notified. Explicit operator-only
GOOGLE_SPONSORED_CREDITS=1 preserves usage/unknown receipts and per-job limits,
never infers authorization from a saved ledger, defaults remain closed/capped.
RED/GREEN tests verify sponsored restart, unknown usage retention, other-model
usage without fabricated pricing, and default refusal.

Actual native API rehearsal job-argonaut-1792959775198 captured three Bevy PNGs
with hashes, then blocked (retained) because Pulkit's new comparison contract
on main exceeded the previously built world-check binary schema. Rebuilt the
current first-party Rust verifier with --locked; no schema downgrade or review
bypass. New job-argonaut-1792959930987 is an explicitly separate zero-spend
rehearsal against that current binary, not a production NeedReport.

Native API v2 completed: job-argonaut-1792959930987, attempt1 repair then
attempt2 accept, both renderSource=bevy-native with3exactimagehashes each.
Visual verdict remains explicitly simulated fixture, no live quality/AC pass.
Python verify_package revalidated this exported package with current first-party
Rust and downloaded/checked all3 nativePNGs successfully. 28Node+36Python tests
pass; actualserver runtime explicitly enables sponsoredcredits. Hourly existing
heartbeat updated from obsolete absent-Studio state to actualcurrentintegration
with source-needs/units blockers and authorizedsponsoredmode. No duplicateworker.
