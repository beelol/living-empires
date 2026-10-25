# Bounded faction action selection implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Steps use checkbox tracking.

**Goal:** Select one feasible, priority-weighted unit action and emit a private, factual GM need report.

**Architecture:** A faction-bound planner consumes a trusted faction-specific view.
Rust validates candidates; a fresh sandbox Lua VM chooses among feasible actions.
Policy replacement stages at a decision boundary with last-good fallback. The
planner returns a proposal/report; existing GM, diplomacy, content and construction owners
retain all scheduling, persistence, service calls and world effects.

**Tech Stack:** Rust 2021 and Lua 5.4; existing anyhow/serde/serde_json/mlua/sha2 only.

**Spec:** docs/work/faction-brain.md

## Global constraints

- No new dependencies, scheduler, world writer, shared save/schema or central exports.
- GM v1 event source=faction-brain; kinds unit_need/blocked_need; ACL only own faction.
- Costs, approval and current conditions are revalidated by authority before effects.
- Main checkpoints use the integration lock, explicit owned files and a normal push.

## Review focus

- Private evidence submitted for another actor must produce no report.
- Stale observation revision must survive in the proposal for authority to reject.
- A blocked high-value action must not suppress a feasible lower-value action.
- Priority multiplication must not overflow or depend on input ordering.
- A purported approval fingerprint must not be confused with an approval decision.

### Task 1: Pure weighted planning/report boundary

Files: create dev/runtime/src/faction_brain/mod.rs, policy.lua and
dev/runtime/tests/faction_brain.rs; update the owned work record.

Produces: plan(actor: &str, view: &FactionView, report_id: &str, at: u64)
-> anyhow::Result<Decision>. Decision contains optional proposed action and optional
GM-v1-shaped report. It never mutates view, calls a model, or commits an action.

- [x] Write tests for swapped defense/hauler weights, actor/evidence isolation,
  blocked/expensive candidates, revision/ACL/report content, idle state and stable ties.
- [x] Run cargo test -p argon-dev --test faction_brain; observe missing-module failure.
- [x] Implement bounded strict input types, validation, deterministic ranking and reports.
- [x] Run focused tests, then cargo test --workspace -- --test-threads=1;
  run targeted rustfmt, clippy and diff inspection.
- [x] Review the whole owned change, record actual results and limitations.
- [ ] Under integration lock, fetch origin/main, reconcile drift, rerun checks needed
  by combined changes, stage only owned paths, inspect staged diff, commit with ACs,
  push normally to main and verify the published revision.

Host exports, real observation ingress, model arbitration, GM ingestion and paid
build execution are a later adapter checkpoint with explicit owner handoffs.
