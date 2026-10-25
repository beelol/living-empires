# Request completion port implementation plan

> For agentic workers: use superpowers:subagent-driven-development or superpowers:executing-plans. Check each completed step against actual evidence.

**Goal:** Complete the user's requests reliably using the working `codex/foundry-fast-iteration` approach, then return to incremental improvement and economy/bots.
**Architecture:** Selectively port completion, renderer delivery and paired startup from `9d23520`/`9f38ad1` onto `0679df0`; retain newer main review-context and reliability fixes. Preserve current saves and frozen historical jobs.
**Tech stack:** Existing Rust, Node and browser renderer; no added dependencies.
**Spec:** Explicit user request to use the approach from “Make colonization single player” first so requests finish, 2026-10-25. Its implementation/evidence is recorded at branch tip `bac3407`, docs/work/foundry-fast-iteration.md. Broader proposed features in ../specs/2026-10-25-progressive-colony-design.md are subsequent work.

## Global constraints

- Keep $1,000 shared session allowance; never enable the branch's uncapped sponsored configuration.
- New style-completion jobs may use 30 candidates/60 calls with no overall time deadline as explicitly requested. Keep ordinary defaults and historical hashed contracts readable. Individual stalled calls remain bounded.
- Only exact independently accepted packages install. Preserve unknown paid outcomes and all failed history; no blind resubmission.
- Separate worker worktrees/indexes and named claims. Parent alone integrates/pushes main and manages current services. Never replace a live provider binary mid-job.
- Source branch is a reference; never replace whole latest-main files if that discards newer changes.

## Review focus

- A terminal blocked Foundry job must not leave a game request polling forever.
- Uncertain POST delivery must not cause duplicate generation or charges.
- Retrying renderer result delivery must reuse exact evidence, not call the creator again.
- Old jobs retain their original cumulative limits and comparison/hash semantics.
- Existing paid user work must finish before service replacement.

## Task 1 — Foundry completion and source identity

Owner: provider_diagnosis / finish-foundry-core. Paths: server.mjs, lib/job-policy.mjs, lib/gemini.mjs, test/job-policy.test.mjs, Rust main/pipeline/provider and pipeline/provider tests, docs/work/finish-foundry-core.md.

Consumes existing AssetPipeline, package and independent review contracts. Produces branch-compatible `completionGoal`, style-completion limits, protocol/capability/source/binary identity fields and remaining retry-group accounting. Coordinate exact JSON names with Task 2 before implementation.

- [ ] Read the complete reference implementation and current-main differences.
- [ ] Import/add regression tests first: 30/60/no-deadline profile, frozen legacy limits, remaining retry-group counters, individual request timeouts and `satisfy-frozen-style` acceptance with no mandatory failures.
- [ ] Observe expected failures against current main; port only compatible implementation.
- [ ] Retain main's component scope, original retained baseline and art-quality prompts.
- [ ] Run full private-target live-provider Rust tests and npm tests. Commit owned paths only.

## Task 2 — Consistent launcher and game recovery

Owner: live_integration_diagnosis / finish-pair-adapter. Paths: scripts/dev.mjs, solo/launch-config.mjs, solo/foundry.mjs, solo/server.mjs, solo/public/app.mjs, relevant existing/new launch/foundry/server tests, solo/README.md, docs/work/finish-pair-adapter.md.

Consumes Task 1's capabilities/protocol/source identity. Produces one same-checkout service pair, stable private live binary, safe terminal request states/recovery controls and unchanged package verification.

- [ ] Read reference files and agree config/health fields with Task 1.
- [ ] Run branch regressions against main to expose stale-service/terminal-job behavior.
- [ ] Port startup handshake, known terminal status handling and per-object regeneration while retaining cancellation/unknown-dispatch protection.
- [ ] Run full Solo/npm suites; isolated test servers only. Commit owned paths.

## Task 3 — Deliver renderer evidence reliably

Owner: renderer_ui_diagnosis / finish-render-delivery. Paths: public/render-delivery.mjs, public/render-worker.mjs, test/render-delivery.test.mjs, docs/work/finish-render-delivery.md.

Consumes existing exact render-task token/attempt/hash payload and server response. Produces bounded retries for the same captured result; no new generation or evidence substitution.

- [ ] Run branch delivery regressions against main; implement the narrow port.
- [ ] Check success, transient failure and final failure without changing payload identity.
- [ ] Run full npm suite; commit owned paths.

## Task 4 — Reproduce and repair live theme-bound failure

Owner: parent / foundry-completion-integration. Paths: lib/theme-inference.mjs, test/theme-inference.test.mjs, test/native-capture.test.mjs, README.md and this stream's design/plan/work record.

Observed normal user windmill request `creation-a2c66d6a-3148-4fb6-905d-e25a5f5dba08` stopped before a job with `Theme shapeLanguage must be 1–500 characters`. A new solar request installed successfully at `0679df0`.

- [ ] Preserve actual error and returned draft metadata; identify byte versus character/schema behavior.
- [ ] Reproduce overlong draft handling with an actual-shaped returned contract before the fix.
- [ ] Implement bounded pre-freeze handling without weakening frozen criteria, dropping required meaning, or issuing hidden paid retries. Preserve original provider response and usage.
- [ ] Fix the independently reproduced native test-fixture startup race: its ready file currently precedes installing its SIGTERM handler. Keep production renderer semantics unchanged.
- [ ] Run relevant/full checks, record the initial failing suite and passing rerun.

## Task 5 — Integrate and prove delivery on main

- [ ] Under one integration lock, fetch/inspect upstream, reconcile exact owned commits and run combined Rust/npm/Solo checks plus renderer rehearsal.
- [ ] Update docs to distinguish ordinary bounded jobs from the explicitly selected style-completion profile; never mark canonical game ACs passed from prototype tests.
- [ ] Wait for live jobs to finish, retain saves, then replace both services and binary together. Verify source identity, capability, dollar cap and stable world.
- [ ] Recover known failed user requests through normal UI, without silently retrying unknown paid outcomes. Observe actual generation, three rendered views, independent review and same-hash installation. Preserve failures and timing.
- [ ] Record exact integrated/pushed revision, screenshots and the video evidence; keep both requested browser tabs open.
