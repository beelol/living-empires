# Delivered world reports in faction diplomacy

Owner: bilal-diplomacy-reports. Baseline 661de33. Primary FAC-02, FAC-06,
DIP-03; affected FAC-09, DIP-14, WORLD-05, EVENT-02, AUD-04, UX-04.
Owned: dev/runtime/src/diplomacy.rs, tests/diplomacy_reports.rs and this record.
The earlier fixture demo was a checkpoint; the resumed goal remains active until
world-aware repeated decisions are demonstrated through the integrated loop.

Dependencies: Argon GM owner supplies public game_master::Event v1 and sole-worker
routing; terrain/G supply actual city facts, legal sites and effects. GM retains
provider, worker and operator page. No parallel scheduler or world executor here.
Current supported diplomatic actions remain the existing seven-field proposals.

Contract explicitly agreed with GM owner: ingest_report(actor, event, received_at),
trusted server-only. Source ACL means permission to receive, not delivery. A route
to one actor must not teach every eligible actor. Persist immutable full envelope
by source event ID plus separate per-actor delivery/time; identical retries are
no-ops, conflicting source IDs fail, and actor must be known and permitted.
Empty ACL cannot deliver. Context shows only facts actually delivered to actor,
without source ACL or other actors' delivery metadata. Ingestion grants no effects.

Knowledge changes invalidate old proposals explicitly: revision is accepted
diplomatic decisions plus distinct delivered reports. Old v1 saves default to
empty report history; new fields are optional but older binaries cannot consume
a save after report delivery. Full evidence survives prompt truncation and restart.
The new data stays in the existing private diplomacy sidecar, never player state.

Verification plan: privacy when two actors are eligible but only one receives;
restart/idempotency/conflicting reuse; stale proposals after learning; rejected
unauthorized/unknown recipients; payload size/depth and atomic failure; prompt
byte bound with intact current treaties. Real file-backed tests and existing
runtime checks, then GM-owned route-to-context integration and bounded live proof.
No fixture or core-only test counts as integrated city behavior or an AC pass.

Checkpoint evidence (2026-10-25): ten file-backed report tests pass, including
real GameMaster route/deliveries interoperability and replay at the 1,024-delivery
limit. Initial missing-implementation failures were retained; independent review
then found deeply nested reports could block the next attempt. A reproduced
depth-122 regression now fails before ingestion; depth 64 survives context,
attempt recording and reload. Read-only review found no remaining introduced
issues. Runtime all-target clippy with warnings denied and owned-file formatting
pass. No provider calls were made by these checks.

On baseline 926cce5, the complete locked workspace test run finished with two
failures: content::native_collection_operations_cannot_bypass_instruction_limits
(one-second child deadline) and the network activation/recovery case (server never
healthy). Both repeat alone; all other suites pass. Workspace clippy also reports
two pre-existing warnings in crates/living-world-core/tests/pipeline.rs (redundant
import and bool comparison). These unowned paths were not changed; coordinator
was notified and failure logs retained under /private/tmp/argon-reports-*.log.
The workspace lockfile is the exact owner-published 926cce5 repair, not a separate
dependency update. Full workspace green and live city behavior are not claimed.

Remaining integration: GM owner imports only actual per-actor deliveries into
the existing sole worker; source owners supply real world observations. Then
verify repeated world-aware model decisions within the existing approved budget.
This checkpoint does not complete the resumed integration goal or any game AC.

Reconciled with origin/main 673aa19, including GM worker checkpoint 927c9da and
terrain/settlement additions. No README/AGENTS drift or owned-path overlap.
Combined checks pass: 34 runtime unit tests, 10 report tests, 14 GM tests, and
runtime all-target clippy with warnings denied. Final diff against that exact
integration base contains only the three claimed files. Main-checkout Studio
work is preserved; publication uses the isolated checkout under the shared lock.

Follow-up owner bilal-diplomacy-preflight, same three paths; primary FAC-02/DIP-03,
affected FAC-06/DIP-14/WORLD-05/EVENT-02. GM owner agreed a pure
can_ingest_report(&self, actor, event, received_at) -> Result<()> with the exact
same validation, duplicate, clock and capacity checks as ingestion. GM preflights
routes before acceptance and owns rejected receipts and continued scheduling.
Reproduced the mismatch with the real ledgers: GM accepted an event whose ACL
contained helios and studio, then faction ingestion rejected that envelope.
No source ACL rewriting, new schema, new worker or additional provider spending.
GM also agreed can_ingest_reports(actor, &[Event], received_at), with single-report
preflight delegating to it. It checks the whole route's distinct new deliveries
against remaining capacity and rejects conflicting IDs within the batch. Pure
preflight does not reserve delivery or guarantee future persistence; accepted
route intent and individual idempotent delivery saves remain GM's recovery path.

Thirteen report tests and runtime all-target clippy pass. Both new public paths
were first exercised with failing stubs. The single-report test's initial
file-absence assumption was corrected: loading a fresh ledger already saves it;
the actual assertion is unchanged saved bytes and no learned knowledge.
The workspace run of the single-report candidate passed other suites but retained
three failures: native_collection_operations_cannot_bypass_instruction_limits,
native_pattern_work_cannot_stall_loading_or_movement (both one-second guards),
and activation_authority_idempotency_reload_and_crash_recovery_over_lightyear
(server never healthy). The later batch delta is covered by the 13-test run.

Combined final batch candidate on main baseline 153aa86: runtime 40, reports 13,
GM 14, content guards 15, and native library 20 all pass; remaining successful
suites are in /private/tmp/argon-preflight-integrated-workspace.log. Full run
finishes with network::activation_authority_idempotency_reload_and_crash_recovery_over_lightyear
(server never healthy) and terrain_network::two_clients_verify_identical_terrain_move_and_reconnect_without_reset
(terrain convergence timeout) failing. These paths are unchanged by this stream.
Earlier failures remain above. Integrated runtime library/report-test clippy and
independent review pass; preflight neither reserves capacity nor promises disk I/O.

Final integration base 99b9fa8 includes public event envelopes and README contract
v2. Read the revised checklist in full. This helper supports FAC-02/FAC-06 knowledge
separation and WORLD-05 recovery; earlier DIP references are historical scope,
not passes for v2's future leader conversations. No event feature or leader-panel
scope is added here. Existing reports remain exact source facts with no effects.
