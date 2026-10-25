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
