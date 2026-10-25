# Main gameplay economy port

Owner: `/root/gameplay_economy_port`; baseline `51ba97ded1333ae9e083f1fa21fe595c73a8eda3`. Selectively ports the behavior from `codex/foundry-fast-iteration` commits `9307bfd`, `238ca89`, and `c2119dc` into main. The source branch's schema-v1 rewrite is excluded.

Primary ACs: SET-04, SET-05, SET-06, GEN-08. Affected ACs: SET-02, SET-03, WORLD-05, UX-04. Owned paths: `solo/model.mjs`, `solo/test/model.test.mjs`, `solo/test/minerals.test.mjs`, `solo/test/factions.test.mjs`, and this record.

Dependency: `/root/gameplay_ui_port` consumes the additive server/UI contract: `creationBlueprint(prompt) -> {function,name,cost,seconds,power,demand,description}`, `buildingDefinition(building)`, `operational(building)`, `alloyRate(world)`, optional fifth `prompt` argument to `canBuild`, and additive `publicState.alloyRate`. Interface was confirmed before implementation. No provider, Foundry, launcher, or AI behavior is included.

Rules: retain main's schema v2 factions, request markers, faction direction adoption, cancellation, receipts/idempotency, and saved-world migration. New colonies start with 100 alloy and earn one alloy per running second. Canonical prompt-selected functions determine fixed costs, work time, demand/output; appearance text and request payload fields cannot set gameplay stats. Consumers reserve power at paid construction; generators remain buildable during shortage. Generated buildings operate only when complete and carrying an installed appearance. Legacy creations derive one function from the saved prompt on validation and freeze it as function version 1 without changing saved alloy, asset, or history. No premium geometry cost is introduced. This Solo prototype remains distinct from Bevy milestone evidence.

Checks: `node --test solo/test/model.test.mjs solo/test/minerals.test.mjs solo/test/factions.test.mjs` — 40/40 passed, including failure/no-mutation and migration regressions. Verify the authority API/UI consumer after integration. Do not treat these local rules/tests as a D0/D1 pass.
