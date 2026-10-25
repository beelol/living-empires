# First Landing

A separate, local **single-player robot colony app**. Click to move, place a solar array, mine alloy, build habitats and fabrication labs, then let Pulkit's Asset Foundry improve the actual city's building appearances. There is no multiplayer transport, shared player session, or Bevy build requirement.

This is the owner's 2026-10-25 alternate direction. It intentionally permits a player robot and manual construction, superseding the root README's camera-only rules **for this app only**. It does not change the other application's contract or claim any v2 acceptance passes.

## Play

Node.js 22+ with npm:

```sh
cd solo
npm install
npm start
```

Open **http://127.0.0.1:4320**. The server binds only to loopback. `SOLO_PORT` selects another port. There is no bundler or compilation step. Everything needed for colony play is local; the renderer uses the isolated Three.js dependency.

- Click dry land to move your robot. Paths go around all building foundations.
- Select a structure or use **1–4**, then click a valid footprint. **Esc** returns to movement.
- **Solar array:** 35 alloy, 8 seconds, +12 power.
- **Mineral extractor:** 60 alloy, 12 seconds, consumes 5 power, extracts 2 alloy/sec from a finite seam.
- **Habitat:** 40 alloy, 10 seconds, consumes 3 power, supports 8 settlers.
- **Fabrication lab:** 70 alloy, 14 seconds, consumes 4 power, recycles 0.5 alloy/sec.
- Right-drag or WASD/arrows pans. Scroll / + / − zooms. Q/E rotates. R finds the robot.
- Click a building to inspect construction, power, function, location and asset provenance. Its appearance can be rolled back.
- Pause stops simulation time. Closing the app server also pauses time; reopening restores progress. Closing only the browser leaves the local simulation running.

A new colony starts with one authored core, one robot and 200 alloy. There are no other empires, combat or manual spawned grants. Construction is charged once and develops visibly over time. Insufficient power pauses consumers; an array can still be built to recover.

## Asset Foundry

The existing Foundry remains a **separate service**, default **http://127.0.0.1:4317**. Start it using the root README if it is not already running. Do not start a second copy against someone else's data directory. Leave its required renderer/Studio worker open. The colony remains playable while Foundry is absent, busy or blocked.

The app automatically queues one bounded job per completed building family per refinement pass. Foundry itself owns creation, renderer review, repair, limits and acceptance. The adapter fetches accepted packages and all three distinct PNGs, runs the first-party Rust `package` operation twice (including typed HTTP-package validation), checks exact package and image hashes, and then installs the appearance for that family. Identity, function, collision footprint, cost, construction, inventory and positions stay in the colony simulation.

The verifier must match the running Foundry. Build `cargo build -p living-world-core --features live-provider` in the Foundry checkout. The app looks in this checkout's `target/debug/world-check`, then the main linked-worktree checkout. Override with **RUST_CORE_BIN=/absolute/path/to/world-check** if necessary.

Default **fixture** mode has **no paid model calls**. It is explicitly **offline rehearsal**: actual renderer images and deterministic checks, simulated visual judgment. Fixture passes can return identical geometry; identical bytes are not counted as new upgrades. This proves plumbing, not a claim that an AI independently improved the art.

To use real generation, the operator must deliberately launch with `SOLO_FOUNDRY_MODE=managed` (or `gemini`) and configure credentials and spending permissions in the **Foundry service**, never the browser. The app does not load keys or weaken Foundry's limits. Live generation/review was not part of the shipped verification; no live paid calls were made for this app.

Use **Pause** to stop new Foundry work; an already-running job may finish. **Cancel job** invalidates local activation before asking Foundry to cancel. **Refine again** starts another explicit pass after existing work finishes. At most twelve jobs are retained per colony session; each individual Foundry job retains its own three-candidate / twenty-minute limits. Unknown dispatch outcomes are reconciled by the exact frozen prompt and provider; they are never blindly retried. **Recheck job** re-reads an existing job and does not create a paid retry.

One fixture job during acceptance was blocked because the upstream native renderer returned non-distinct views. Its failure is retained. The app did not install that package. Other actual fixture packages completed and installed successfully.

## Durable, independent integration

`SOLO_DATA_DIR=/private/path` chooses a separate colony; default `solo/.data/`. This directory is ignored, private and contains an atomically written, flushed `world.json`, a single-writer lock and retained `assets/<content hash>/` packages, job records and exact reviewed PNGs. Preserve it when switching revisions. Invalid saves fail visibly instead of resetting. An unexpected process death can lose at most the last periodic background simulation tick (one second); acknowledged commands are flushed before success. Unknown paid dispatch is saved before HTTP. Host downtime is paused simulation time.

`SOLO_FOUNDRY_URL` selects a loopback Foundry. This app does not write the Bevy save, protocol, Lua policies or Foundry implementation. It interprets checked Foundry recipes as **visuals** inside a fixed authoritative foundation envelope. Robot traversal treats that whole envelope as solid, including during construction; it does not claim native/Studio interior-traversal parity. Authored starters and reviewed generated appearances remain distinguishable in inspection.

All integration uses ordinary JSON over HTTP, so another language or engine can replace either side:

| Method and route | Contract |
| --- | --- |
| `GET /api/v1/health` | API/save version, app identity, configured Foundry mode. |
| `GET /api/v1/world` | Read-only current render snapshot, resources, robot route, buildings, assets, Foundry outcomes. |
| `POST /api/v1/command` | `{"id":"stable-operation-id","type":"build","kind":"solar","x":12,"z":8}`; also `move` with x/z and `pause` with boolean `paused`. Replaying the exact semantic request returns its original result. Changed reuse fails. |
| `POST /api/v1/foundry` | `toggle` with `enabled`, `next`, `rollback` with `kind`, or `retry` / `cancel` with `requestId`. Only the server has activation authority. |
| `GET /api/v1/export` | Download the complete colony save, including command receipts. |

Requests use `Content-Type: application/json`; the browser Origin must match the printed loopback URL. The API rejects unknown/oversized commands, invalid kinds, unsafe sites, duplicate entity IDs and unaffordable builds. A local trusted CLI can omit Origin. This is one local player's app, not an exposed multi-user service.

## Verify

```sh
npm test
```

The tests cover durable HTTP actions, exact-once costs, restart, invalid terrain, finite resources/power, pathing, concurrent Foundry work and player commands, cancellation during review, changed packages/images, unknown dispatch reconciliation and rollback preserving gameplay. See `evidence/acceptance.md` for actual browser steps and screenshots. Tests do not establish the other application's world milestone criteria.
