# Gameplay economy and UI port

- **Owner:** `/root/gameplay_ui_port` (gameplay UI/API stream)
- **Base:** `51ba97ded1333ae9e083f1fa21fe595c73a8eda3`
- **Primary ACs:** SET-04, SET-05, SET-06, GEN-08, UX-04
- **Affected ACs:** SET-02, SET-03, WORLD-05
- **Owned paths:** `solo/server.mjs`, `solo/public/app.mjs`, `solo/public/index.html`, `solo/public/scene.mjs`, `solo/test/server.test.mjs`, `solo/test/economy-api.test.mjs`, `solo/README.md`, `docs/work/gameplay-economy-ui-main.md`
- **Dependency owner:** `/root/gameplay_economy_port`; commit `9d1d8d7972f745ee430f58bf662fa91e21ed38b0` delivers the agreed local economy model v1 interface: `creationBlueprint(prompt)`, `buildingDefinition(building)`, `operational(building)`, `alloyRate(world)`, prompt-aware fifth argument to `canBuild(world, kind, x, z, prompt?)`, and `publicState.alloyRate`. The UI consumes the declared preview/build interfaces; model/persistence schemas remain model-owner territory.
- **Boundary:** port canonical prompt cost/function preview, alloy income display, prompt-aware ghost/placement and inspector function display. Add `POST /api/v1/blueprint`; preserve all existing Foundry, health, identity, hash, retry/reconciliation, cancellation, target-scope and rollback behavior.
- **Checks:** `node --check` on changed JavaScript modules; `node --test solo/test/economy-api.test.mjs solo/test/server.test.mjs` after the dependency commit is available. These implementation checks do not pass integrated game ACs.
- **Evidence/status:** implementation and focused source edits are ready; source-worktree syntax and ownership checks passed. The real API tests are awaiting the parent integration checkout, where the model commit can be combined with the latest main revision. No browser/gameplay acceptance run requested or claimed. Existing saves retain their resource balance; only the new-colony fixture changes to 100 alloy under the model dependency.
