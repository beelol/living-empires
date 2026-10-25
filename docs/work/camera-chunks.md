# Camera chunks and scope retirement

Owner: Sol, `/private/tmp/argon-camera-chunks`; baseline fd982be.
Primary: EXP-01/03/09/10. Affected: PLY-04, SET-02, WORLD-05, UX-03/05.
Owned initially: README.md, AGENTS.md, docs/archive/game-contract-v2.md, this record, camera-terrain/. Solo integration requires explicit handoff from bilal-solo-colony.

User direction supersedes v2: retire multiplayer/autonomous empires and make terrain/foliage/rocks follow the camera in the new single-player app. Retire requirements, preserve all code/history/dirty work and active claims. Root contract v3 is now the only active checklist. No criterion is automatically passed.

Dependencies: single-player chat owns `solo/`, scene and server static routes; Foundry owner retains HTTP/package and live credentials. Existing model v1 `coast(x)`, dry-land/collision rules and save v1 remain unchanged. Terrain is cosmetic; no resource grants, deposits, simulation changes or paid calls. Preserve existing groundY geography. Deterministic seed/version describes visual details only.

Plan: (1) archive v2 and replace active scope; (2) test deterministic chunk data, shared edges/normals, bounded streaming and clearance; (3) implement disposable instanced renderer and camera-footprint coverage; (4) hand off scene integration, run solo suite and actual browser travel/revisit with screenshots; (5) serialize integration with single-player owner. Checks and evidence pending.

Explicit handoff: single-player owner released scene.mjs and the three new files from bilal-solo-colony. Root docs belong to recovery01a174b4; my draft is preserved at /private/tmp/argon-scope-retirement-draft/ for adoption. No root-doc edits in my candidate. Retired old work was already stopped/archived by recovery; no claim deletion.
Owned now: solo/public/scene.mjs, chunks.mjs, terrain.mjs, solo/test/chunks.test.mjs, this record. Latest uncommitted scene copied exactly from owning solo worktree before edits; pre-change copy /private/tmp/argon-scene-before-chunks.mjs.

## Validation so far

- `node --test solo/test/chunks.test.mjs`: 6/6 pass. Missing-module failure observed before implementation; tests cover shared world-space vertices/normals across negative chunk boundaries, deterministic details, existing ground-height compatibility, view rectangle coverage, exact disposal counts over 20 km simulated travel, and decoration clearance including prop radius.
- Full single-player suite in isolated copied preview: 29/29 pass, including the six new checks. Initial restricted run failed three HTTP tests with `listen EPERM`; the authorized loopback rerun passed all 29. This was an environment failure, retained here.
- Actual browser at `http://127.0.0.1:4325`, fresh `/private/tmp/argon-chunks-preview-save`, Foundry automatic jobs disabled. Normal controls: D held 20 seconds (ocean beyond original extent), S held 20 seconds (distant inland), two zoom-out presses, Q held 3 seconds, R to return. Continuous terrain/water and grounded rock/foliage detail seen; original core, robot and mineral markers visible on return. Desktop and narrow viewport screenshots observed. No generation calls triggered by terrain tests.
- Actual screenshots: `/private/tmp/argon-camera-chunks-inland.png`, `/private/tmp/argon-camera-chunks-return.png`. Visual spot check only; no reference hardware/frame timing or full v3 integrated acceptance claimed. No resource extraction/depletion test was performed in this browser run; simulation unaffected and covered by existing tests.
- Resident chunk counts/disposal verified in deterministic unit travel, not measured through GPU telemetry. Actual 20-minute performance audit remains not_run.

Renderer contract: 64 m chunks, 2 m world-space terrain sampling, per-chunk coordinate PRNG seed 1337 / terrain visual version 1. Analytic finite-difference world normals eliminate independently computed edge normals. Instanced props use shared geometry/materials, per-chunk instance buffers; unloading disposes chunk geometry, surf geometry and instance resources, preserving shared resources. Camera frustum corners project onto y=-2 with a 16 m margin, covering rotation/aspect/zoom. Camera-following sun maintains lighting/shadows remotely. Colony state and collision/save/package formats unchanged. Water remains the existing color/material at sea level, rendered per resident chunk.

Final candidate baseline: bae60c194859b530decc6779339f72eb83d53ee6 (initial single-player checkpoint). Reclaimed scene only after owner recorded exact original scene and released it. `cd solo && npm test` on this exact checkout: 29 passed, 0 failed. `git diff --check`: clean. Additional A-held travel after rotation verified continuous inland visuals across negative coordinates. No root docs in candidate; their writer acknowledged draft handoff.

Independent read-only review: no blocking findings. Reviewer checked camera coverage, winding/normals, deterministic regeneration, instance capacities, clearance and disposal. Residual limit: long-session GPU memory/frame timing not measured.
