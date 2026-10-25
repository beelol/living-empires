# Living Empires operating contract

Current deliverable: **asset generator**. Living Empires is the downstream product context; the current milestone is one themed asset generation–review–repair–export lifecycle. Traversal is only a minimal asset-validation harness. Preserve the 122-criterion Living Empires contract in README.md as the long-term context; do not build unrelated game features before this asset lifecycle is proven.

## Coordination

- Sol is the integration manager. Use bounded Luna workers when delegated work helps; preserve existing work and commits.
- Every worker task declares owned files, the interface it delivers, an acceptance command, and required evidence. Report the result and commit SHA.
- Work on `main` as explicitly requested. Serialize named-file commits; never stage another worker's unfinished files or switch shared branches.
- User-authorized milestone commits may push normally to `origin/main` after acceptance checks. Never force-push; record the synced commit only after the push succeeds.
- Keep scope updates concise: shipped scope, gates passed/failed/blocked/not_run, exact blocker, next outcome.

## Acceptance and repair

- Freeze each job's theme, brief, criteria version, and limits before generation. Creators cannot edit the judge or weaken a frozen contract.
- Reproduce the failure, make the smallest useful repair, run affected checks and regressions, and retain before/after evidence.
- Rust `living-world-core` owns deterministic validation. The browser uses matching collision-aware movement; a parity failure blocks acceptance.
- Capture the exact candidate in three actual renderer views. A live reviewer must inspect those images separately from the creator. Generated illustrations are not renderer evidence.
- Retain candidate/evidence hashes and reject stale, duplicate, cancelled, or changed packages at activation.
- Keep three candidate attempts and twenty minutes per ordinary job. API retries/review count toward the call ceiling. A call ceiling is not a monetary spending cap; obtain an explicit spending ceiling before extending live paid testing. Unknown paid outcomes require reconciliation before retry.
- Report fixture generation/review as rehearsal. Do not label a busy-skipped smoke test passed, or any missing/inconclusive check passed.

## Credentials and release

- `.env`, `.data/`, `node_modules/`, and `target/` are ignored. Never print or commit credentials. Keep keys server-side.
- Publishing, public deployment, crate publication, and expanded credential or cost permissions are not implied by a local implementation or commit request.
- Activate only exact accepted candidate/evidence hashes. Preserve the prior compatible release for rollback.

## Current gates

1. `cargo build -p living-world-core` and `cargo test -p living-world-core`.
2. `npm test` for adapter, acceptance, and Rust/browser parity checks.
3. With the local Studio running, `npm run test:e2e` for offline rehearsal, real renderer captures, repair, activation, cancellation, and rollback.
4. Separately demonstrate live Gemini creation plus independent image review, preserving usage/session metadata and before/after evidence. Live creator success alone is not a completed loop.
