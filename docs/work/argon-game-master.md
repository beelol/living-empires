# Argon game master

Owner: bilal-argon-gm. Base 347c860. Primary FAC-02 FAC-09 UX-04 WORLD-05;
affected AUD-04 DIP-14 EVENT-02 EVENT-04 LIVE-01. Owned: game_master/,
tests/game_master.rs, this record and own implementation plan. No AC pass claimed.

User requested an agentic layer sending game activity to a persistent Argon game
master that decides and distributes information; then explicitly asked to set a
goal, continue, and push working checkpoints to main. Proceeding inline.

Design: keep faction minds separate. GM receives authenticated server-side event
facts and reports in a separate single-writer durable ledger. Immutable event IDs
are deduplicated with conflicting reuse rejected. A GM proposal cites known event
IDs. Routing copies exact facts only to recipients allowed by ALL cited events;
model summaries stay operator-only. Event/content requests become pending proposals,
not activated game effects. Simulation validation and independent release checks
remain authoritative. Record inputs, model, proposal and accepted/rejected result.
Persist before acknowledgment; restart cannot repeat accepted deliveries.

Approaches: reusing faction context would leak knowledge; a second diplomacy writer
would race existing consent. Chosen separate GM ledger with explicit bridge into
existing sole writer and world-event sources. Existing protocol/save schemas unchanged.

Dependencies: gemini-host owns Gemini transport and diplomacy_loop v1; requested
handoff/wiring contract. Studio/replay owner owns source event history, asked for
envelope agreement. Living-events owns event scheduling/validation; requested adapter.
No shared interface change until agreement. Core can be built independently.

Checks: real file-backed ingestion/dedup/restart, source conflict, recipient privacy,
stale model responses, failure recovery, atomic failed write, writer exclusion,
corrupt-save refusal, bounded input and queue. Then host integration and live model
attempt only when adapter and budget settled. Tests alone do not pass README audits.

Checkpoint 1 evidence:
- Initial three tests failed against explicit unimplemented ledger, then passed.
- Future observed revision save regression failed, then passed after validation fix.
- Independent core review found deep JSON could acknowledge an unloadable save and
  post-rename directory-sync failure could leave writer behind disk. Both real-file
  regressions failed before fixes and pass after candidate decode and writer stop.
- Nine GM tests pass; rustfmt and runtime all-target clippy pass.
- Existing runtime library 28 tests, CLI 1, host 16 pass. Broader runtime suite
  retains content one-second subprocess deadline failures in
  native_collection_operations_cannot_bypass_instruction_limits,
  native_pattern_work_cannot_stall_loading_or_movement,
  special_files_cannot_block_content_loading. First also failed isolated retry.
  No content implementation or timing threshold changed by GM; owner notified.
- This checkpoint is infrastructure, not a live GM, D0 proof or any AC pass.
  Export/provider/scheduler bridge remains next. Pending requests do not activate
  content/events. Full-game event clocks and paid-build effects belong to owners.

Contracts agreed: Studio export-gm per-recipient arrays preserve event ACLs and
UNIX-second at. Faction-brain NeedReport matches GM v1 with own-faction ACL.
Living-events public snapshot includes only orbital broadcasts and explicit
observations; no private ledger import. Same diplomacy worker will count combined
persisted core + GM attempts against max_calls; faction rotation and diplomacy
request IDs remain based on core attempts, GM IDs separate. No duplicate scheduler.

Ruling: user said continue; implement inline without further design approval.
Ruling: preserve existing worktree edits, transfer only owned new files under main
integration lock, following newer coordinated direct-main direction.

Checkpoint 2 in progress: durable attempts persist model/input before spend and
complete result atomically; interrupted requests become explicit failures on load.
Shared worker budget = diplomacy attempts + GM attempts, IDs/rotation independent.
Studio intake .gm-inbox.json is bounded1MiB/1024events and transactionally dedups;
ledger .game-master.json preserves all IDs and blocks at total1024events, rather
than dropping history. Continuous long-world indexing remains a later dependency.
Thirteen tests pass including real Diplomacy::apply -> GM facts with original ACL,
never importing private proposal summaries. GM transport local HTTP test passes
with separate role and only request_content/wait supported initially. No live
provider call or operator UI deployment yet. Existing loop handed off explicitly
by diplomacy-limits owner after05cf47d. Terrain owns additive lib export.

Runtime connection checkpoint evidence:
- Separate Argon role uses existing bounded direct Gemini generateContent transport;
  it is NOT a Managed Agents integration. GM supports request_content/wait initially.
- Same worker schedules one GM turn after a faction round; combined durable
  core+GM attempts obey max_calls. A reproduced old-world backlog-spending bug was
  fixed by persisting the faction turn handled by each GM attempt.
- Independent review found duplicate Studio work under fresh request IDs. Regression
  failed before canonical evidence-set/target dedup, then passed across restart.
- Candidate c1f5161 plus six owned files: 34 library tests,14 GM integration tests,
  remaining runtime suites pass (113 cases). The previously failing one-second
  content subprocess guard passes in isolated retry; retain the initial failure.
- Headless native suite:8 library,1 CLI,2 real Lightyear network tests pass.
  Runtime clippy, owned-file rustfmt, headless build and actual HTTP page JS syntax
  checks pass. Whole-workspace fmt reports pre-existing events.rs/tests/events.rs
  formatting; those paths remain untouched and their owner is notified.
- Actual localhost4328 test using configured gemini-4-argon completed exactly4 calls:
  3 faction +1 GM, zero provider failures, one accepted pending content request
  citing gm-smoke-city-need-1. Input explicitly says fixture; this is connectivity
  evidence, NOT a real city need/build, D0, or a full-game AC audit.
- Full input/proposal/outcome evidence remains /private/tmp/argon-gm-live/
  world.json.game-master.json and world.json.diplomacy.json. HTTP API returns GM
  events, attempts, decisions and combined counts; private operator-only data is
  not added to native player snapshots. Browser control failed twice with
  transport closed, so actual visual/screenshot QA remains blocked.
- Studio owner accepted sidecar outbox: accepted request_content receipts resolve
  cited exact source events and their ACL; stable receipt ID prevents repeated
  generation. No model summary can activate content or grant paid construction.
- Follow-up agreed with diplomacy owner: ingest_report(actor,event,received_at)
  records eligibility separately from actual per-faction delivery. Initial GM
  provider remains content-only until that independently tested API lands.
