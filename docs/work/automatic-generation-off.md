# Automatic generation off for ordinary construction

- **Owner:** Codex Solo bugfix stream
- **Base:** `50ffbd815f5eb949f6070d3d80b701627ce0e096` (`origin/main` at worktree creation)
- **Primary ACs:** GEN-08, UX-04
- **Affected ACs:** SET-04, SET-05, LIVE-10, WORLD-05
- **Owned paths:** `solo/model.mjs`, `solo/foundry.mjs`, `solo/test/model.test.mjs`, `solo/test/factions.test.mjs`, `solo/test/foundry.test.mjs`, `solo/test/server.test.mjs`, this record
- **Dependency:** Root integration manager; interface is Solo save schema v2, unchanged.
- **Contract:** The four premade building kinds (solar, extractor, habitat, workshop) never queue art from ordinary construction, including area or explicit-faction placements, and are excluded from background discovery. Explicit freeform creation/regeneration and explicit faction style/assignment actions remain available while automatic work is off. Exact approved packages still install normally; disabled automatic work does not automatically adopt or propagate faction art. When enabled, existing automatic adoption behavior remains.
- **Cross-cutting rules:** Preserve authority, exact-once construction costs, explicit intent, exact-package approval, cancellation/recovery evidence, and save compatibility. No Foundry service/provider/renderer/limit/cost or schema changes.
- **Acceptance command:** `node --test solo/test/*.test.mjs`
- **Required evidence:** Red failure reproducing construction enqueue; green premade true/false and discovery cases; explicit freeform/faction actions with automatic work disabled; accepted install without direction propagation; retry/restart coverage from the existing Solo tests.
- **Integration state:** Implementation and integration checks pending.
