# First Landing

The default single-player robot colony: click to move and build, or **Create something → type a prompt → click a spot**. A standard 8 × 8 metre foundation and fabrication volume appear immediately. Live Asset Foundry generates and independently reviews the requested appearance, then the exact accepted package replaces that object's placeholder. Each prompt is independent; there is no fixed catalog of permitted subjects.

## Run

```sh
npm install
just dev
# Equivalent: npm run dev
```

Game: **http://127.0.0.1:4320**. Foundry: **http://127.0.0.1:4317**. `npm run studio` starts/checks Foundry alone. Both bind loopback. The launcher starts a missing Foundry from this checkout with a private `world-check` binary, then verifies the game and Foundry source, protocol, model assignments and binary identity before reusing them. A different checkout on a configured port is preserved; choose separate ports to run another checkout. It reads the main checkout's `.env.local` from a linked worktree; configuration precedence is explicit environment → `.env.local` → `.env`. `ARGON_ENV_FILE` overrides the local file. Keys stay server-side and are never returned to the browser.

Normal startup uses configured credentials and spending limits without enabling sponsored credits. New style-completion jobs allow up to 30 candidates and 60 counted API calls, stop on independent acceptance, and have no overall job deadline; individual requests remain bounded, and the explicit dollar allowance is separate from these call limits. Previously saved jobs retain their original frozen limits. **Offline generation is disabled in the game.** Historical rehearsal receipts remain labeled; they are never relabeled as live evidence.

The stable save is the main checkout's **`.local/solo-colony/`**, shared across this app's branch/main launches, not the old Bevy worlds. The earlier prototype save is migrated once without overwriting an existing colony. Never open two authorities against one save. Foundry's existing `.data/` is separate and preserved.

## Play

Click dry land to move; paths avoid building foundations. Keys 1–4 choose a structure; Esc returns to movement. Right-drag or WASD/arrows pans; scroll / + / − zooms; Q/E rotates; R returns to the robot. Terrain/rocks/foliage stream around the camera with bounded resident geometry. There is no survey fence. Water and invalid/overlapping sites remain invalid.

Solar arrays cost 35 alloy, take 8 seconds and supply 12 power. Extractors cost 60 alloy, take 12 seconds, use 5 power and extract 2 alloy/sec from a finite seam. **Find & build a mineral extractor** chooses an affordable accessible procedural seam and uses the same construction validation. Habitats cost 40 alloy, take 10 seconds, use 3 power and add 8 to the robot capacity. Fabrication labs cost 70 alloy, take 14 seconds, use 4 power and recycle 0.5 alloy/sec. Prompt-created plots cost 25 alloy and take 6 seconds to construct; live generation continues independently.

A new colony starts with a core, one robot and 200 alloy. There is no multiplayer, other empire, combat or free model-issued world grant. New robot recruitment/actor packages are not yet implemented; the capacity counter is a real limit, not a claim that extra robots have spawned.

## Live generation and repair

The adapter resolves a live, prompt-led artistic contract before freezing the generation job, including its palette. Inference is told the actual renderer capabilities: axis-aligned boxes, vertical cylinders/cones, and three flat palette tokens; it must not invent requirements for unsupported rotations or texture maps. Generation is schema-enforced Gemini by default, with managed generation also available. The configured service uses `gemini-4-argon` from `.env.local`.

Foundry owns generation, actual three-view capture, independent visual judgment, repair and acceptance. The app reruns the first-party Rust package operation, reparses the HTTP package through that typed validator, checks identity/content hashes and every distinct PNG hash, then enforces the actual rendered shape bounds against its fixed plot. Unsupported or changed packages stay blocked. Rehearsal, missing evidence or failed review never becomes a live pass.

Appearance changes preserve entity identity, coordinates, construction, resources and function. The fabrication volume does not stretch a generated object. Candidate renders appear in the selected building inspector as unapproved previews; only the exact independently accepted package installs. Failed/cancelled jobs remain inspectable. A terminal job stays in history; **Regenerate with live Foundry** creates an explicit new contract revision for the same funded plot without another alloy construction charge. An uncertain dispatch must be reconciled before any new paid work; the adapter never blindly retries it. Automatic family improvements have a separate pause toggle; one explicit prompt does not resume paused automatic work.

Unknown paid dispatch outcomes are reconciled against the original Foundry history before retry. A refused connection is queued without pretending a paid request occurred. Service repair waits for active paid work to finish, retains the previous binary and exact saved outcomes, and never clears the city. A generation failure leaves the last compatible appearance/placeholder intact.

The adapter expects the current Foundry package contract and a matching `world-check` binary. Build `cargo build -p living-world-core --features live-provider` in the service checkout, or set `RUST_CORE_BIN` explicitly. Do not replace a running service's binary mid-job. No Foundry API or acceptance gate is weakened by the colony adapter.

## HTTP integration

All boundaries use versioned JSON/HTTP, so another language or engine can replace the app. `GET /api/v1/world` returns the current render snapshot and Foundry URL; `GET /api/v1/health` identifies API v1/save v2 and the running checkout source hash. `POST /api/v1/command` accepts a stable `id`, `type` (`move`, `build`, `pause`, `prospect`, and faction setup actions) and validated arguments. Builds accept `kind`, x/z; `creation` additionally requires a free-form `prompt` and live `provider`. Exact semantic retries return the prior result; changed operation reuse fails without spending.

`POST /api/v1/foundry` handles scheduling, cancellation, reconciliation, rollback and explicit regeneration. Only the local authority can install an approved package. `GET /api/v1/export` downloads the complete colony. Requests require JSON and a matching browser Origin. Private state, keys and provider responses are not exposed as empire knowledge or public content.

## Verify

```sh
node --test solo/test/*.test.mjs
npm test
cargo test -p living-world-core
```

Tests cover costs/idempotency, finite resources, power, site/collision/pathing, save recovery, cross-origin rejection, correct configuration precedence, camera streaming, live inferred palette propagation, per-object activation, paused scheduling, exact packages/images, cancellation races and fixed plot bounds. Actual live generation outcomes and retained failures are recorded in `docs/work/solo-colony.md`; code tests alone do not pass game acceptance criteria.
