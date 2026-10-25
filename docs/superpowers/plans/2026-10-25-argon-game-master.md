# Argon Game Master Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task.

**Goal:** Durable Argon GM event intake, decisions and permitted information routing.
**Architecture:** Separate single-writer ledger beside faction diplomacy; exact event
facts are routable, free model summaries remain operator-only; side effects proposed.
**Tech Stack:** Existing Rust, serde_json, fs2, Gemini transport; no new dependencies.
**Spec:** docs/work/argon-game-master.md

## Global Constraints
- Preserve faction privacy, consent and simulation authority.
- No protocol/commander save changes. No content or rules self-activation.
- Working checkpoints pushed to main after reconciliation and checks.

## Review Focus
- Conflicting event/request IDs must fail without changing history.
- Routing multi-event decisions uses intersection of permissions.
- Stale provider results cannot act after new input/restart.
- Disk failure must not acknowledge or mutate in-memory state.
- Durable evidence must remain bounded without losing dedup identities.

### Task 1: Durable GM ledger
Files: src/game_master/mod.rs, tests/game_master.rs (under dev/runtime).
Produces: GameMaster::load(path), ingest(Event), context(), decide(id, revision,
proposal), deliveries(recipient), snapshot(). Typed Event/Proposal boundary.
- [ ] Write file-backed tests for ingestion/restart/conflicting reuse and privacy.
- [ ] Run cargo test -p argon-dev --test game_master; verify feature-missing failures.
- [ ] Implement bounded transactional ledger and exact event routing.
- [ ] Verify targeted tests, fmt/clippy and runtime suite; commit verified checkpoint.

### Task 2: Runtime bridge and Argon decisions
Files subject to explicit owner handoff; record final paths before editing.
Consumes Task 1 ledger, existing sole diplomacy loop and settled source adapters.
- [ ] Add bridge/provider failure and restart tests first; observe failure.
- [ ] Integrate event intake and model request with bounded budgets and asynchronous work.
- [ ] Expose operator evidence; no private context in player snapshots.
- [ ] Run host/runtime checks and permitted live service check; retain failures.
- [ ] Review, reconcile current main under lock, verify and push working checkpoint.
