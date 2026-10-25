# Single-player Colony Implementation Plan
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
**Goal:** Finish a separate playable robot colony and Foundry integration within one hour.
**Architecture:** Node owns a local durable simulation and versioned JSON API. Three.js renders it. A separate controller consumes Pulkit's existing HTTP Foundry without changing its implementation.
**Tech Stack:** Node standard library, Three.js, node:test, existing Playwright for browser verification.
**Spec:** solo/PRODUCT.md; owner's 2026-10-25 direction supersedes camera-only scope for this separate app.
## Global Constraints
Only solo/ and named work/plan records. No multiplayer dependencies or shared schemas. Authority validates cost/site/idempotency. Keep exact reviewed bytes and evidence. Default zero-spend rehearsal. Never claim v2 AC passes.
## Review Focus
- Duplicate/concurrent construction: one debit/entity per operation and footprint.
- Invalid terrain or trapped route: no paid invalid placement, collision-aware robot movement.
- Save restart: identity, resources, construction, assets, unknown Foundry dispatch survive.
- Unavailable/busy/cancelled/stale Foundry: play continues, no implicit retry spending or unapproved activation.
- Upgrade/rollback: preserve world effects, verify first-party package and three evidence hashes.
### Task 1: Durable colony rules
Files: solo/model.mjs, solo/store.mjs, solo/test/model.test.mjs.
Interface: createWorld(), command(world, request), advance(world, seconds), publicState(world); atomic single-writer store.
- [ ] Write and run failing tests for cost, overlap, invalid land, robot pathing, production/power, duplicate IDs, snapshot recovery.
- [ ] Implement rules and atomic save; run node --test solo/test/model.test.mjs.
### Task 2: HTTP authority and Foundry adapter
Files: solo/server.mjs, solo/foundry.mjs, solo/test/server.test.mjs, solo/test/foundry.test.mjs.
Consumes Task 1. Produces GET /api/v1/world, POST /api/v1/command and /api/v1/foundry, GET /api/v1/health. Every command has a stable operation ID. Server owns background ticks and journaled jobs.
- [ ] Write failing tests for HTTP persistence, origin checks, offline Foundry, package/evidence mismatch, restart dispatch reconciliation.
- [ ] Implement serial local API, bounded Foundry jobs, typed first-party verification, hash-linked imports and rollback. Test with node --test solo/test/*.test.mjs.
### Task 3: Playable world and verification
Files: solo/public/*, solo/package.json, solo/README.md, solo/test/browser.mjs, solo/evidence/*, docs/work/solo-colony.md.
Consumes Task 2 and existing package schema. Delivers click movement/build ghost/construction/resources/inspection/Foundry status, pause/zoom/pan, automatic save, automatic family upgrades.
- [ ] Build the 3D scene and HUD; install only isolated dependencies.
- [ ] Exercise real browser move/build/invalid placement/reload/Foundry upgrade/rollback and narrow viewport; preserve screenshots.
- [ ] Run tests and independent final review; fix material findings, commit owned paths on codex/solo-colony and report revision, commands, limitations.
