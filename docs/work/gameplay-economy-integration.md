# Gameplay-only economy port onto main

Owner: parent/integrator. Baseline 51ba97ded1333ae9e083f1fa21fe595c73a8eda3.
Primary SET-04/05/06, GEN-08 and UX-04; affected SET-02/03, LIVE-10, WORLD-05.
This is the explicitly requested Solo browser prototype, not Bevy D0/D1 evidence.

The user requested working gameplay from the referenced Make colonization single
player chat, excluding its Foundry changes. Source baseline bac3407, gameplay
commits 9307bfd, 10515bc, 238ca89 and evidence c2119dc. Preserve main's newer
factions, areas, adoption, progress, cancellation and exact package activation.
No AI: the user explicitly deferred it. Do not reset the shared dollar ledger.

Economy worker bilal-gameplay-economy-port owns model, model/mineral/faction tests
and docs/work/gameplay-economy-main.md. API/UI worker owns Solo server, app/index/
scene, server/economy-API tests, Solo README and its own work record. Parent owns
this integration record and existing completion plan/evidence notes. All streams
use isolated worktrees and claims. No other chat is messaged or edited.

Stable interface: creationBlueprint(prompt) returns fixed function/cost/time/
power/demand/name/description; buildingDefinition(building) and operational(b)
respect saved authoritative function; canBuild accepts optional fifth prompt;
alloyRate(world) and publicState.alloyRate expose actual rate; view.setTool accepts
the same prompt. The API blueprint route is read-only and cannot grant resources.

Port scope: new worlds start with 100 alloy and earn one per simulation second;
active extractors add two while their finite seam lasts. Generated solar, wind
and power plants supply twelve only after paid construction and approved art.
Consumer power is reserved at placement; insufficient alloy/power changes nothing.
Existing balances/assets/identities/faction history remain readable and paid.
Geometry-based premium pricing, bot factories, and autonomous AI are deferred.

The user then explicitly requested a fresh colony and a push if verification
passes. After combined checks/review, back up the existing world and start a
fresh one using normal world creation, preserving all Foundry assets/history and
the existing $1,000 ledger. Preserve active paid outcomes; do not silently retry.
The user subsequently explicitly requested opening the fresh game in their browser.
Open it once after startup; do not repeatedly raise windows. Runtime/API evidence
is distinct from actual browser evidence.

Gates pending: model/faction/mineral regressions; actual API cost/function/power
checks; combined Solo/npm/Rust checks appropriate to touched code; fresh-world
restart persistence; existing asset-function migration; final source review;
latest-main reconciliation/ownership check; normal fast-forward push. Keep any
failed checks and report exact running/published revisions.

## Reconciliation and checks

Integrated model 9d1d8d7 as 8491b13 and UI/API 013c8dc as 71601e6 onto
fd81c469bae9f44b97b84a6013185409fc581d76. Reconciled the later main
3da99d25fc1bf53d23fe59ccae96a48f62d6a520 progress-message fix; retained newer
faction adoption, role separation, full-brief inference, and progress targets.
No Foundry source-branch changes or AI were imported in this gameplay stream.

Initial combined Solo result: 117/118. The newer faction adoption fixture assumed
the old starting balance; explicitly funded only that multi-building fixture.
Independent review found legacy generated extractors lacked seam bindings. Added
a failing regression, then bound only existing nearby saved seams, including
exhausted seams, without creating geography. Off-seam old extractors remain
nonproducing and the inspector identifies the missing seam. Repair d640178.

After repair: Solo 119/119, npm adapter/parity 72/72, zero skipped. Independent
review found no remaining important issue and separately ran model 17/17. A copy
of the real 34-building save retained identities, balances, approved assets,
factions, deposits, receipts and history. A real isolated server began at 100,
earned 1.11079379199998 alloy in about 1.2 seconds, and retained its ID/balance on
restart. Proof /tmp/gameplay-save-proof.json. Rust code is unchanged from the
previously verified 51ba97d binary (core70, npm72, renderer rehearsal recorded in
foundry-request-completion-main.md); this is not a new paid generation proof.

The old paid propagation job finished and installed before shutdown. No paid job
was cancelled. Full idle snapshot:
/Users/bilal/projects/argon-empires/.local/foundry-main-verification/completion-upgrade-1792970824786.
Fresh reset will archive the entire old Solo directory; shared Foundry assets,
job history, and the $1,000 ledger remain intact. Browser/startup checks follow
publication. These implementation checks do not establish Bevy D0/D1 or claim
that every live generation will succeed.
