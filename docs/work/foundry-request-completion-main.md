# Request completion on main

Owner: Sol/integrator. Baseline `0679df026ea2faf41753d1075ea5382bace332f6`.
Primary LIVE-03/05/10 and UX-04; affected GEN-02/08 and WORLD-05. This is the
browser Solo/Studio slice, not canonical Bevy D0/D1 evidence.

The user requested the completion approach from “Make colonization single player”
(`codex/foundry-fast-iteration`, final branch record `bac3407`) on main, explicitly
prioritizing requests finishing before background improvement and economy/bots.
The broader design remains a deferred draft. Port only reviewed owned changes;
retain main's newer art direction, scoped review and retained-baseline context.

## Ownership and interfaces

Parent owns theme inference/tests, the native test fixture, README and this
stream's notes. `finish-foundry-core` owns server/core/provider policy and config
identity. `finish-pair-adapter` owns launcher/Solo adapter/server/UI compatibility.
`foundry-render-delivery-01` owns renderer delivery. All use separate worktrees.
No cross-chat message or edits to another stream's economy/model/scene claim.

The branch's explicitly requested new style-completion profile permits 30
candidates/60 calls and no overall deadline. Historical frozen contracts and
ordinary Rust defaults retain their original limits. Individual calls remain
bounded; unknown paid outcomes retain reservations. The runtime session keeps
its authorized $1,000 allowance, never the reference branch's uncapped setting.

## Observed failure and smallest repair

On main `0679df0`, user windmill request
`creation-a2c66d6a-3148-4fb6-905d-e25a5f5dba08` failed before a job because its
returned shapeLanguage was 584 bytes, exceeding the 500-byte contract field.
The complete invalid draft and usage were already retained in
`theme-draft-d350898c-092e-4ddb-846d-e1f3bf740d3a.json`. Its building still said
queued; the adapter owner is fixing that inconsistent terminal display.

Before freezing an inferred draft, excess descriptive prose is now wrapped into
labelled rendering notes. Every word remains in the full contract; the raw
response/usage are retained. Identity, exclusions and visual criteria are never
rewritten. If preserving all text would exceed the notes capacity, fail with the
original returned contract rather than truncate it. This performs no second
provider request. Malformed palettes and other invalid contracts still fail.

Replaying the actual returned windmill draft locally now validates with one
continuation note and unchanged visual checks. This is replay evidence, not a
new live generation pass. Focused tests failed first on the original failure,
then passed for exact prose retention, UTF-8 bounds and full-note failure.

The baseline npm suite also exposed an intermittent native-renderer fixture race:
its `started` file was written before its SIGTERM handler existed. A cancellation
could correctly terminate the child before the fixture wrote `term-received`.
Move only the fixture readiness write after handler installation. Production
termination and evidence behavior are unchanged.

## Checks and evidence so far

- Initial main build/live-provider Rust suite: 65 passed.
- Initial npm: 58 passed, one cancellation fixture failure retained. Focused
  rerun 14 passed and full rerun 59 passed before the fixture adjustment.
- Initial Solo: 39 passed.
- Parent repair: 13 focused theme/contract tests passed; full npm 62 passed,
  zero failures/skips. Logs `/tmp/foundry-completion-parent-npm.log` and
  `/tmp/foundry-theme-wrap-red.log`; actual returned-draft replay proof at
  `/tmp/foundry-theme-wrap-actual-proof.json`.
- User's new solar request on `0679df0` accepted and installed with one candidate
  (`job-helios-stepped-solar-array-1792968356328`). Earlier windmill failure retained.

Combined integration, renderer rehearsal and recovery of the failed user plots
remain not_run until recorded below. Do not infer acceptance from worker tests.

## Integration reconciliation and independent review

Integration moved to `foundry-completion-main`, based on fetched `86bacde`.
Parent/renderer/adapter owners explicitly handed off immutable commits and
released claims; only handed-off paths are edited. Incoming faction areas,
per-building design progress, exact verified design reuse and shared-job
cancellation protection are retained. Conflicts in the adapter were combined,
not replaced with the older branch files. Model, scene, area UI and saved-world
schema remain upstream-owned and unchanged by this stream.

Independent scoped reviews accepted renderer delivery and parent inference/UI
repairs. Adapter review requested stronger loaded Node source and game verifier
identity checks, plus blocking regeneration of uncertain paid theme requests.
Three regressions failed before repair and passed after it. The launcher now
compares the loaded Foundry Node fingerprint, and game health exposes its private
verifier path/hash. Transport uncertainty is distinguished from returned invalid
theme contracts; uncertain paid work remains blocked for reconciliation. Explicit
regeneration of known terminal failures preserves prior outcomes and charges no
second construction fee. Cancelling during inference also preserves uncertainty.

Combined Solo suite after reconciliation: 85/85 passed, zero skips. Log:
`/tmp/foundry-combined-solo.log`. These are prototype implementation checks,
not canonical world acceptance. Core/provider schema port and combined live
verification remain pending until evidence below.
