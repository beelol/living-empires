# Foundry request completion core

Owner: provider_diagnosis (`finish-foundry-core`). Baseline: `a47f993` (reconciled from the exact upstream revision selected by the integration owner).

Primary acceptance areas: LIVE-03, LIVE-04, LIVE-05, LIVE-06, LIVE-10. Affected areas: GEN-02, GEN-08, UX-04, UX-05, WORLD-05. These are behavior boundaries, not checked-off milestone evidence.

Owned paths: `server.mjs`, `lib/job-policy.mjs`, `test/job-policy.test.mjs`, `lib/gemini.mjs`, `crates/living-world-core/src/main.rs`, `crates/living-world-core/src/pipeline.rs`, `crates/living-world-core/src/provider.rs`, `crates/living-world-core/tests/pipeline.rs`, `crates/living-world-core/tests/provider.rs`, and this record. No changes are planned in `lib/gemini.mjs` or the native provider request path: the active server uses Rust provider calls, and the reconciled main's Argon JSON MIME-only transport, complete prompt schema, local validation, and retained provider error/usage behavior remain intact.

## Interfaces and dependencies

- `AssetPipeline` freezes the new creation profile as 30 candidate attempts, 60 provider calls and `timeBudgetMs: 0` (no overall job deadline). Its ordinary defaults remain three attempts, six calls and twenty minutes, so historical contracts continue to validate with their original limits/deadlines.
- New non-refinement Studio creation jobs freeze `completionGoal: "satisfy-frozen-style"`. The loop stops at the first independently reviewed, fully passing candidate. An unchanged comparison can pass only when there are no defects/regressions/findings and every deterministic, renderer, theme and collision gate passes. Explicit saved-baseline refinements do not receive this goal and retain comparative improvement and frozen component-scope checks.
- Retries only accept reconciled incomplete responses. They inherit the original job group's remaining attempt/call counters and original finite deadline, if any; null-deadline groups stay null-deadline. Unknown outcomes remain reserved and are not retried automatically.
- Individual network/provider and renderer operations remain bounded. A no-deadline job removes the overall timer; it does not remove provider request bounds, renderer bounds, cancellation or spend reconciliation.
- The Solo adapter owner `live_integration_diagnosis` consumes `/api/config` fields `service`, `sourceRoot`, `sourceRevision`, `protocolVersion`, `packageVersion`, `rustBinarySha256`, `roleModels`, and Rust `capabilities` (`nativeProvider`, `pipelineVersion`, `packageVersion`, `maxAttempts`, `maxProviderCalls`, `noDeadline`, `styleCompletion`, `providerFormat`, `sourceSha256`). The config also exposes `jobLimits` and `completionGoal` as new-job UI defaults. Historical jobs must display their own frozen contract limits.
- The parent owns `lib/theme-inference.mjs`, its tests, and `README.md`. Its theme-inference normalization may add optional `draft.normalizations`; this server persists the list with successful and invalid drafts, returns it from `/api/themes/resolve`, and retains returned usage/response for invalid drafts without changing existing theme/draft fields. The renderer-delivery owner keeps exact captured evidence and retries its delivery only.
- The parent retains the authorized $1,000 session ceiling. This stream does not enable sponsored/uncapped billing, issue provider calls, alter a running service, or treat historical unknown outcomes as reusable.

## Checks

- `node --test test/job-policy.test.mjs test/gemini.test.mjs`: passed (11 tests).
- `npm test`: the current run remained blocked by the separately claimed `test/native-capture.test.mjs` fixture startup race: two full reruns each passed 62/63, with the timeout test and cancellation test alternating a missing `started` sentinel. The root has assigned that fixture race to its owner; this stream did not edit those files.
- `cargo test -p living-world-core --bin world-check`: passed (4 tests).
- `cargo test -p living-world-core --features live-provider`: passed (70 tests across library, CLI, HTTP transport, pipeline and provider targets; doc-tests: 0).
- The focused pipeline policy and completion tests passed, including profile bounds, no-deadline call ceilings, unchanged style acceptance only after all gates pass, and failure on regressions. Rust provider/review behavior remained unchanged.
- `node --check server.mjs` passed. No paid generation/review, live service replacement, actual renderer acceptance, world integration, or release activation was run here. LIVE/GEN/FLOW milestone checks remain unclaimed.
