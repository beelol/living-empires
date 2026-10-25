# Foundry native capture diagnostics

- Owner: `foundry-native-diagnostics`
- Base: `e94a2a5dea9250f83bb3cd5a7e1fe6ef601b65ba`
- Primary ACs: LIVE-10 (stop safely), LIVE-04 (retain actual renderer evidence)
- Affected ACs: LIVE-06 (renderer failure remains unapproved)
- Owned paths: `lib/native-capture.mjs`, `test/native-capture.test.mjs`, this record
- Dependency/interface: preserve the current `captureNative` result and candidate/evidence hash contract; native `foundry_audit` CLI remains `PACKAGE.json OUTPUT_DIR`.
- Scope: retain bounded native renderer diagnostics for spawn/exit/timeout/cancel failures; terminate and reap the subprocess before rejecting; stage native images, backups and evidence metadata before public publication; roll back prior images after publication errors and report incomplete rollback.
- Checks: `node --test test/native-capture.test.mjs`; `npm test`.

## Evidence

- Baseline focused tests: 5 passed.
- RED: five new real-subprocess cases fail against the baseline: failed renderer stderr is not saved; timed-out child is not waited for/killed; active cancellation does not wait for exit; pre-aborted signal still reaches spawn and no cancellation log is written; spawn failure diagnostics are not saved.
- GREEN: `node --test test/native-capture.test.mjs` passed 10/10, including timeout/cancel escalation, abort-before-start, bounded failure logs and unchanged public evidence.
- `CARGO_TARGET_DIR=/tmp/foundry-native-diagnostics-target cargo build -p living-world-core` succeeded. The first `npm test` lacked the ignored `target/debug/world-check` binary and failed 5 tests for `ENOENT`; with `RUST_CORE_BIN=/tmp/foundry-native-diagnostics-target/debug/world-check npm test`, all 39 tests passed.
- Follow-up RED: four injected-filesystem cases failed before publication staging: manifest write failure left replaced images; second-frame rename failure was not observed because publication used writes; new frames had no rollback; rollback failure was not surfaced.
- Follow-up GREEN: `node --test test/native-capture.test.mjs` passed 14/14. Injected manifest-stage failure preserves all prior bytes; second-frame failure restores prior bytes or removes newly created frames; rollback failure reports the affected target and retained backup path.
- Final full suite: `RUST_CORE_BIN=/tmp/foundry-native-diagnostics-target/debug/world-check npm test` passed 43/43 after the follow-up.
- No provider calls, live service actions, or renderer binaries were run.
- Residual limitation: subprocess lifecycle was exercised with real Node fixture processes, not the actual Bevy renderer binary or a live Studio run. Multi-file publication is rolled back for detected errors; power loss or process crash during sequential renames is outside this change.
