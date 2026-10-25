# Agent working agreement

## Read the whole specification first

`README.md` is the canonical local milestone acceptance checklist (contract v2). Before changing
behavior, read **all** of it, including audit rules and integrated milestone scenarios;
do not work from a keyword extract or just the assigned AC. Re-read relevant
changes whenever the baseline advances. The linked
[game specification](https://chatgpt.com/space/page_9d7940a6aa5c8191bc081c561105cfbb)
is context, not a second local checklist to maintain. Flag any contradiction
between a requested change and an existing criterion before implementing it.

For each stream, record primary AC IDs, other affected AC IDs, owned paths,
dependencies, shared interfaces, and checks in its task or per-stream work record.
Inspect existing implementations and active claims before choosing an approach.
Name the owner and expected interface/version of each dependency; if unavailable,
report blocked behavior honestly and build only work whose contract is settled.
An AC is a behavior boundary, not exclusive ownership of every file related to it.

## Preserve these rules across every subsystem

| Rule | ACs to consider when changing it |
| --- | --- |
| The authoritative simulation validates ownership, costs, consent and idempotency; camera/UI/model proposals and assets cannot grant world effects. | SET-01–06, SET-08–10, SET-13, GEN-07, DIP-05, DIP-14, WORLD-01, WORLD-03–05 |
| Empire minds receive only their permitted observations/history. Observer-wide inspection is not empire knowledge. | FAC-02, FAC-06, DIP-03, WORLD-01, UX-04, EVENT-02 |
| Acknowledged durable actions recover once; identity, resources, geography, orders and history survive updates/restarts. | EXP-09–10, GEN-08, LIVE-08–11, WORLD-03–05 |
| Only independently checked exact files may activate; provenance, images, budgets, cancellation and rollback remain linked. | GEN-01–03, GEN-05–08, LIVE-03–11, LIVE-13, UX-03–04 |
| Placement, access, civilian traversal and terrain changes agree with authoritative collision and the active content version. | SET-01–03, SET-08–10, GEN-05–06, LIVE-08–09, LIVE-13, EVENT-02–03 |

These are review prompts, not a replacement for the full checklist. Changes to a
shared action schema, persistence format, asset format, permissions, clocks, or
collision contract require coordination with all affected stream owners before
implementation. Prefer compatible additions; never quietly change another
stream's assumptions to make your own test pass.

The `dev/` playground may execute local rules for iteration, but those rules are
not independently approved game releases. Do not treat development activation or
its tests as satisfying LIVE release checks or the integrated milestone gate.

## Isolate, claim, and keep changes small

`game/` is the Bevy/Lightyear shell; `dev/runtime/` owns the Lua and durable-world
core. Network messages/channels in `game/src/protocol.rs` must register identically
on both sides. Coordinate protocol versions, saved-world formats and authoritative
rules before changing them. Server auto-deployment does not update other devices'
client binaries. A local development hot reload is not an approved LIVE release.

- Every concurrent stream uses its own Git worktree and index. Never run two
  agents' Git operations in the same checkout. Temporary branches or detached
  commits are permitted; completed work integrates directly into `main`.
- Claim a small path set before editing using `dev/agent.py`. List claims first;
  discuss shared ACs and contracts even when paths do not overlap. See
  [the workflow](dev/AGENTS-WORKFLOW.md) for commands and multiple-computer limits.
- Do not edit outside your claim. Split work or arrange an explicit handoff when
  ownership must change. Release only your own claim after integration or a
  recorded handoff. A stale-looking claim is not permission to take it.
- Avoid broad formatting, incidental cleanup, and regeneration of unrelated
  files. Shared configuration, dependency locks, central exports, schemas, and
  README edits need a named owner and serialized changes.
- Inspect status before every Git mutation. Never reset, clean, restore, discard,
  stash, amend, or stage another stream's work. Unexpected dirty files are a
  reason to stop that operation and identify their owner, not to tidy them away.
- Stage explicit owned paths, inspect the staged diff, and commit a small coherent
  change with AC references. Never use blanket staging in a shared context.

## Integrate into main in order

One process at a time may use a clone's integration checkout. Hold the local
integration lock for the **whole** fetch → reconcile → check → commit → push
sequence. Cross-computer ownership agreements are also required; local locks do
not coordinate separate clones.

Fetch `origin/main`, inspect upstream changes, and integrate against that exact
revision. If it moved, reconcile in your isolated workspace, re-evaluate affected
ACs and contracts, and rerun the checks the combined change needs. Check both
uncommitted paths and the final committed diff against the integration base.
Run the project's available checks and meaningful tests of the touched behavior;
include targeted failure/retry/concurrency cases when those rules change.

Push only a normal fast-forward update to `main`; never force-push it. A rejected
push means another stream won the race: fetch again, reconcile, inspect, and
retest before retrying. Do not blindly repeat the push or overwrite the remote.
Do not rename remote branches, change repository settings, publish messages, or
alter branch protection without authorization. If `origin/main` does not exist,
report the setup gap rather than inventing a different integration target.

## Report evidence, not completion by assertion

Report changed behavior, primary/affected ACs, checks and outcomes, unresolved
dependencies, and integration revision. Implementation tests alone do not check
off game ACs. Only the README audit protocol's required setup, normal action path,
authoritative records, and actual client evidence can establish a pass. Missing
dependencies or evidence mean **blocked**, never passed. Keep failed outcomes.

## Asset generator lifecycle

# Living Empires operating contract

Current deliverable: **camera-only autonomous city growth plus the asset lifecycle**. Use three starter empires (organic, cyborg, robot), two independent observer views, the Rust/Bevy/Lightyear game with bounded Lua policies, and a separate asset-only Studio lane. Follow the F0 → D0 → D1 milestones in README contract v2: empires choose buildings, civilian units and valid sites; cities visibly form over time; needed designs pass generation, independent renderer review, repair, approval and paid construction/use. The standalone asset generator remains a required slice, not the whole milestone. Significant-event notices are optional polish. The final stretch is talking to generated races through leader panels. Combat, avatars, human factions and manual empire control are outside all active milestones and stretch scope.

The old 122-criterion contract is preserved only in `docs/archive/game-contract-v1.md`. It is historical context, not a second checklist or active backlog. Reassess old evidence and stream records against v2; never reuse an old pass automatically. This scope change does not itself authorize changes to runtime schemas, persistence or collision contracts. Existing stream owners retain their claims; coordinate any implementation changes with them.

## Coordination

- Sol is the integration manager. Use bounded Luna workers when delegated work helps; preserve existing work and commits.
- Every worker task declares owned files, the interface it delivers, an acceptance command, and required evidence. Report the result and commit SHA.
- Work on `main` as explicitly requested. Serialize named-file commits; never stage another worker's unfinished files or switch shared branches.
- User-authorized milestone commits may push normally to `origin/main` after acceptance checks. Never force-push; record the synced commit only after the push succeeds.
- Keep scope updates concise: shipped scope, gates passed/failed/blocked/not_run, exact blocker, next outcome.

## Asset-lane acceptance and repair

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

## Asset-lane gates

These commands verify only the asset tooling slice. World acceptance requires the README core and FLOW scenarios in the actual Rust/Bevy/Lightyear game, with bounded Lua policies. Passing these commands does not establish D0 or D1.

1. `cargo build -p living-world-core` and `cargo test -p living-world-core`.
2. `npm test` for adapter, acceptance, and Rust/browser parity checks.
3. With the local Studio running, `npm run test:e2e` for offline rehearsal, real renderer captures, repair, activation, cancellation, and rollback.
4. Separately demonstrate live Gemini creation plus independent image review, preserving usage/session metadata and before/after evidence. Live creator success alone is not a completed loop.
