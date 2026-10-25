# Living Empires

Watch autonomous empires form cities on an alien coast. The human controls a flying camera: move, look, zoom, inspect and follow. Empires decide which buildings and units they need, commission missing designs, choose valid locations, pay for construction and put the results to work. Cities visibly take shape over time.

**Active acceptance contract: v2, 2026-10-25.** This README is the canonical checklist, aligned with the [world-first milestone plan](https://docs.google.com/document/d/1zPXxhRmADCMbM2gYi_8fgPdDuKpMX8fIhdPIchYGV_A/edit?tab=t.0) and the owner's camera-only scope correction. Combat, avatars, human-controlled factions, personal inventories, manual unit orders and player construction are removed from scope, including stretch scope. The final stretch is talking to generated races through leader panels. Significant-event notifications are optional polish.

The [122-criterion v1 contract](docs/archive/game-contract-v1.md) is historical context only, not an additional completion gate. Retained IDs below keep their subject area; their revised requirements belong to v2. Previous passes and old work records must be reassessed against the new wording. Retired IDs are not reused. This scope revision changes documentation, not runtime behavior or existing evidence.

## Milestones and owners

| Priority | Demonstrable outcome | Owner / acceptance gate |
| --- | --- | --- |
| **F0 — Foundations** | One coastal scene, two independent observer cameras, three persistent empire identities, a sample building/unit kit, autonomous choices, and input-driven asset repair with actual renders. | G: world/camera; E: empire decisions; A: asset loop. AUD-01–04, PLY-04, EXP-01–03, FAC-01–02, FAC-12, GEN-01–02, LIVE-03–06, UX-01, UX-03–04. Label fixtures. |
| **D0 — Complete city loop** | Empire need → building or unit brief → generate → render → independently inspect → repair → approve → unlock → paid construction/production → useful work. Agents choose valid building sites; both views show cities growing over time. Demonstrate one new building and one new unit, not just a mesh swap. | All three owners. Every required criterion in the core sections below must pass; FLOW-01 and FLOW-02 prove integration. Target hour 3, use hour 4 to stabilize; elapsed targets are planning goals, not measured performance. |
| **D1 — Repeat and sustain** | A changed economic need produces a different decision and a second useful content cycle. All three empires keep growing while no viewer is connected; reconnect/restart retain the result. | E + A lead, G commits authority. FLOW-04; re-run affected core checks. No new feature takes priority over a working D0. |
| **Optional polish — Notifications and events** | Concise notices for a real empire decision, completed landmark or event start; one event prompts a visible city response. | G: display/authority; E: response. EVENT-01–03. Not required for D0 or D1. |
| **Final stretch — Talk to races** | From camera mode, open a generated race's leader panel, ask about its city and needs, and receive grounded replies while cities continue running. | E: conversation; G: panel/validation. DIP-01–03, DIP-14. Optional bounded agreements add DIP-04–05 only. Nothing later depends on an avatar. |

**Three parallel tracks:** G (Bilal) owns the Rust/Bevy/Lightyear world and terrain, observer clients, authoritative state, persistence and activation. E (empire teammate; Bilal if two people) owns identities, observations, goals, building/unit choices, siting proposals and content needs. A (asset colleague) owns generation, real render review, repair and package submission. E proposes; G validates and commits; A creates. Changeable empire/city policies live in bounded Lua; Rust retains authority and persistence. The Node/Three.js Studio is the separate asset-validation lane, not the game client. The standalone asset loop can advance alongside city foundations; neither fixture is the integrated D0 proof.

## Run the world prototype

See the [development guide](dev/README.md) for the current Bevy/Lightyear host, client and local update commands. `cargo dev` is the sprint launch entrypoint. Existing development controls or demos may expose older behavior; their presence does not make it part of this contract. Two windows are observer views of one authority, not two human factions. No public deployment is implied.

## Run the asset Studio

Requires Node.js 22+, npm, a stable Rust toolchain, and a Gemini API key for live generation. The local server binds only to loopback; no public deployment is configured.

```sh
npm install
npm run build:core
cp .env.example .env
# Set GEMINI_API_KEY in .env. Never commit this file.
npm run dev
```

Open the printed local URL (default `http://127.0.0.1:4317`) in the in-app browser and keep it open: the Studio browser is the primary asset renderer. **Chromium installation is optional** and only needed to capture without an open Studio. Choose a theme and edit the brief. Use **Gemini managed agent** for the intended managed workflow, **Gemini direct** for the smaller recipe-generation path, or **Offline rehearsal** for a clearly labeled fixture demonstration without API calls. Both live modes use a separate Gemini image review. The theme freezes when a job starts; generated geometry must use its saved material palette, and visual review also checks architectural/biome consistency.

Optional headless-renderer setup:

```sh
PLAYWRIGHT_BROWSERS_PATH=.data/browsers npx playwright install chromium
# AUDIT_RENDERER=headless npm run dev
```

The optional challenge checkbox inserts a blocked entrance after initial generation. The runner records three actual rendered views, the Rust core rejects blocked traversal, and the repair receives the observed failure. Click **Activate** only after the candidate passes all six demo gates. Download the approved asset package for game consumption: `GET /api/jobs/{id}/package` returns a versioned recipe, frozen theme, manifest, required checks, content hash, and evidence hashes. Rehearsal packages carry `manifest.rehearsal: true`. Rollback becomes available after two releases. Run limits are three candidates, six live API calls, and twenty minutes per job; this is a call ceiling, not a guaranteed monetary spending cap. There are no hidden request retries.

Keep the Studio open while running: its browser worker can render the exact job/attempt/hash offscreen and return three actual PNG views plus a collision-aware movement trace. The controller validates the token and Rust parity, fingerprints evidence, and rejects stale/cancelled submissions. This fallback avoids blocking on a headless-browser download. Headless capture is used when no Studio worker is connected; `AUDIT_RENDERER=headless` forces it. The visible asset preview stays separate from audit rendering.

```sh
npm run test:core
npm test
# With the local server running; uses fixture generation/review, real Rust checks and game renders:
npm run test:e2e
```

Our [`living-world-core`](crates/living-world-core/README.md) crate contains internal `provider` and `pipeline` modules alongside recipe validation and collision simulation. `provider` constructs Gemini requests, parses replies, supplies a native optional HTTP transport and owns fixture generation/repair. `pipeline::AssetPipeline` freezes and hashes contracts, counts attempts/calls, accepts independent evidence, decides repair/approval, enforces cancellation and produces `ApprovedAssetPackage`. The `world-check` JSON boundary exposes create, fixture, generate, review, submit, charge, decide, cancel and package operations. Node does not independently approve new jobs; it asks Rust and uses Rust-returned hashes. Legacy saved packages remain readable through a compatibility path. The app build uses `--features live-provider`; the offline library can also build without network transport. Build output goes in ignored `target/`; the crate and Cargo lockfile stay here. `RUST_CORE_BIN` can override the binary path.

Runs, rejected versions, provider responses, screenshots, and release manifests persist under ignored `.data/`. On restart, an unfinished paid operation is marked blocked for reconciliation. Credentials stay in ignored `.env` and are sent only from the backend to Google's API. This Studio is an asset-lifecycle harness. Its rehearsal results do not prove autonomous city growth or the integrated Bevy milestone.

## Shared contracts and acceptance rules

Agree each boundary before implementation; this revision does not change a network schema, save format, collision rule or asset reader. Record the actual version used in every job and audit.

| Interface | Owner → consumer | Required meaning |
| --- | --- | --- |
| EmpireSpec | E → G + A | Stable empire/race IDs, style, goals, observations, resources, available building/unit designs and saved decision weights. |
| Decision / build proposal | E → G | Source state/tick, chosen design and purpose, proposed site or unit role, cost/cap prerequisites, operation ID. Authority rechecks current legality. |
| ContentNeed | E → A | Empire, building or unit, unmet purpose, site constraints, budget, source decision and deduplication key. |
| Render request/result | A ↔ G | Candidate hash, scene/seed, camera and frame IDs; actual Bevy views for world integration. Studio-only renders prove only the Studio slice. |
| AssetPackage / activation receipt | A → G → E | Frozen contract, format, role, footprint/access, bounded cost/stats, exact files and evidence hashes; approved manifest and activation tick unlock a design. They do not grant a free entity. |
| WorldEvent / notification | G → E + observer | Stable event ID, actor, tick, location, actual outcome and permitted observers. Camera visibility never becomes empire knowledge automatically. |

Every job freezes its theme, brief, criteria version, supported role, limits and source state before creation. Creator and reviewer are separate; only the authoritative controller approves exact checked bytes. Ordinary jobs allow **three candidates total and twenty minutes**, with reviewer calls, API retries and failures counting toward the configured call ceiling. Set an explicit spending ceiling before live paid work. Unknown paid outcomes require reconciliation before retry; cancellation or restarting cannot reset limits. Keep the previous compatible release for rollback.

`living-world-core` owns deterministic asset validation; the world host owns city rules and durable effects. Browser movement must match Rust collision in the Studio harness. The Bevy integration must additionally demonstrate its own placement, access and movement using the same declared asset properties. A Studio pass alone cannot establish Bevy parity.

### Audit protocol

All boxes start unchecked. A criterion is a requirement, not a claim that it is built. Report **pass, fail, blocked, not_run**. Missing fixtures, dependencies, frames or inconclusive observations are blocked, never passed. A skipped or busy smoke test is not a pass.

For each result retain contract v2, criterion ID, exact code and asset revisions, world seed, starting snapshot, normal action/proposal path, expected/actual effects, authoritative records, timestamps and actual client images. Records prove ownership, costs, orders and persistence; images prove the visible scene. Neither substitutes for the other. Camera actions cannot create resources or issue empire commands. Setup fixtures may establish state, but cannot perform the behavior under test.

Audit terrain on three independently chosen seeds. Repeat FAC-03, FAC-05 and FAC-12 on five reset variants selected after the release is fixed; at least four must achieve the stated outcome and every run must obey authority, knowledge, resources and exact-once accounting. Keep all failures. Fixed-state decision scoring and recorded accepted commands must replay; fresh model calls need not return identical text. Fixtures/replayed providers are rehearsal; live claims require actual provider/session/usage metadata. A managed-agent claim requires substantive managed generation and repair; label direct-model fallback accurately.

Use real time for responsiveness and provider latency; simulation time can advance construction and economy only through their normal tick path. Freeze test hardware, rules, client builds, clocks, asset format and budgets. At 1920 × 1080 on an Apple M2 with 16 GB, medium settings, audit three empires, 100 buildings and 200 civilian units for 20 minutes: at least 95% of frames within 33.3 ms and no stall over one second. For two-view synchronization use 50 ms round-trip latency and 1% packet loss; record the host hardware. Thresholds are acceptance targets, not current measurements.

## Core ACs — observer world

- [ ] **AUD-01 — Restore independent setups.** Save and restore terrain, empire resources/observations, designs, orders, clocks and active assets for each criterion without replaying another criterion. Test actions still use normal validation.
- [ ] **AUD-02 — Observe commands and effects.** Automation can operate the camera, inspect a city and distinguish proposed, accepted, running, completed and rejected empire actions; it can inspect authority and capture the actual renderer.
- [ ] **AUD-03 — Retain traceable evidence.** Reopening a result retrieves its exact contract, release, candidate/evidence hashes, source snapshot, sequence and outcomes. A failed or unrun requirement never appears passed.
- [ ] **AUD-04 — Replay rules honestly.** Recorded accepted commands reproduce costs, placement, production and ownership from the same snapshot. Live decision/generation runs retain actual calls and accepted results; replay and fixtures are labeled.
- [ ] **PLY-04 — Observe with a flying camera.** Each viewer can pan, rotate, zoom, inspect a building/unit and follow or recenter on a chosen city/unit. Controls create no avatar, faction, orders or world effects and cannot move the other camera.
- [ ] **EXP-01 — Share a continuous landscape.** Two observer clients see the same coast, landforms, resources and city coordinates without tile gaps, unexplained seams or jumps between showcase scenes.
- [ ] **EXP-02 — Provide readable city terrain.** One coast and adjoining buildable land contain valid connected city sites and routes. The camera can inspect all three empires and their workers. Extra biomes and a four-square-kilometre map are not required.
- [ ] **EXP-03 — Ground the scene.** Coast-to-inland transitions, water, terrain and vegetation read coherently in actual views. City foundations meet terrain; no floating props, exposed seams or square material boundaries on audited routes.
- [ ] **EXP-09 — Preserve places after unloading.** Fly away until a developed area unloads, then return in both views. Buildings, remaining resources and routes retain their saved positions/state without refill or duplication.
- [ ] **EXP-10 — Keep established geography stable.** Compatible terrain/content updates preserve existing city sites and saved changes; new areas connect without gaps. Intentional geographic changes require a recorded world event.

## Core ACs — cities and agentic decisions

- [ ] **FAC-01 — Keep three distinct empires.** Three autonomous empires (organic, cyborg and robot) have saved IDs, race/style, motives, city anchors, resources and design rosters. Reconnect, restart and art updates preserve their identities and histories.
- [ ] **FAC-02 — Separate empire minds.** Each empire records its own permitted observations, goal, decision, proposed action and actual outcome. Private state or a discovery given to one cannot enter either other empire's prompt/history without a recorded observation or report.
- [ ] **FAC-03 — Solve a city need.** With a resource bottleneck, known valid site and sufficient prerequisite resources, an empire starts a feasible building or civilian-unit plan within 90 real seconds and improves the bottleneck within ten simulation minutes. Explanations without action do not pass.
- [ ] **FAC-05 — Plan city growth and revise it.** An empire completes at least two dependent actions, such as produce a builder then construct a needed building. If the intended site becomes invalid after the first order, it starts a feasible alternative within 90 seconds instead of repeating the rejected placement.
- [ ] **FAC-06 — Use actual knowledge.** An unobserved deposit, other empire's private decision or unreported event cannot influence factual claims or a targeted order. Recording an observation/report makes only its supplied facts available.
- [ ] **FAC-09 — Keep the city running without a model.** During a five-minute service outage, accepted construction, civilian movement and production continue normally. Show the decision pause; on recovery revalidate delayed proposals against current state before committing them.
- [ ] **FAC-12 — Let priorities change building and unit choices.** From equivalent affordable options, different saved needs/weights lead to different useful investments, such as storage versus a courier or production versus housing. Swapping only priorities swaps the preference. Show legal candidates, key scores, selected design/site, reason and actual result; fixed state/rules/tie-breaks repeat.
- [ ] **SET-01 — Form cities over time.** From a declared minimal colony kit, each empire autonomously adds at least three completed buildings serving at least two distinct functions during a 20-simulation-minute run. Timestamped start/middle/end views show a recognizable growing cluster with access routes and activity; a prebuilt city or instant decorative scatter does not pass.
- [ ] **SET-02 — Choose valid building sites.** Agents propose locations using terrain, existing footprints, access, resource proximity and rights. Excessive slope, deep water, overlap, blocked access and another empire's land without rights are rejected without charge. A valid alternative is selected and built; neither a camera click nor an art job chooses ownership.
- [ ] **SET-03 — Show construction before use.** A funded accepted order creates a visible site, progresses over its declared work time under its power/worker rules, and becomes usable only at completion. Remove a prerequisite to pause it; restore it to resume remaining work.
- [ ] **SET-04 — Account exactly once.** Construction/production reserves or debits the displayed cost once. Duplicate start, completion and cancellation messages cannot duplicate entities, refunds or charges. Concurrent proposals for one site produce at most one accepted building; balances stay nonnegative.
- [ ] **SET-05 — Make resource production real.** A working extractor consumes a finite deposit at its declared rate and credits actual storage by that amount. Exhaustion, full storage or missing power pauses it with a reason; restart does not replenish resources.
- [ ] **SET-06 — Make power constrain choices.** Generation and demand match displayed rates. Shortage pauses/slows declared consumers; an empire can choose additional supply. Restoring power resumes work without granting unpaid progress.
- [ ] **SET-08 — Produce useful civilian units.** An empire chooses and pays for a worker, builder or courier, respects build time/cap and places it at a valid traversable exit. It performs its assigned city/logistics function visibly. A blocked exit waits; approved design availability alone cannot spawn a unit.
- [ ] **SET-09 — Deliver actual goods.** A courier removes a declared quantity from source storage into cargo, follows a valid route and credits the destination only on arrival. Repeated messages, cancellation and reconnect never duplicate cargo.
- [ ] **SET-10 — Handle blocked access.** Blocking an active construction access path or delivery route causes a valid reroute or an explicit pause. Dependent city work consumes only actual stock. Reopening resumes the remaining work/delivery once.
- [ ] **SET-13 — Expand without rewriting ownership.** New buildings and city extensions use affordable valid sites and explicit construction rights. A new design, marker or model response cannot seize another empire's land or silently relocate existing structures.

## Core ACs — generate, review, build and use

- [ ] **GEN-01 — Distinguish city styles.** The three empires have saved architectural/material rules and visibly different building silhouettes or construction forms. With names and faction colors hidden, an independent reviewer can distinguish the three city families in actual views.
- [ ] **GEN-02 — Extend a coherent family.** Newly generated building and civilian-unit designs follow the requesting empire's frozen shape, material and scale rules in the city scene. An independent reviewer checks those rules; recoloring an unrelated kit is insufficient.
- [ ] **GEN-03 — Make generated units functional.** A new civilian-unit design has an owner, supported role, movement/collision and required work animation. After paid production, the unit completes a real city task; a preview or static mesh is insufficient.
- [ ] **GEN-05 — Make generated buildings functional.** A new building design declares purpose, cost, work time, footprint, access/collision and supported function. After approval the empire chooses a valid site, pays, constructs it and demonstrably uses its function in the city.
- [ ] **GEN-06 — Fit buildings and units to the site.** Validate a generated building on flat and legal sloped sites selected after generation starts. Supports meet terrain, access connects to routes and declared unit classes traverse it in actual clients. Rust/renderer collision disagreement blocks acceptance.
- [ ] **GEN-07 — Commission from a real need.** A recorded empire decision creates a bounded building/unit job when the current roster cannot meet its need. Approval unlocks only the exact accepted design; the empire then pays, builds and uses it through ordinary rules. Existing city activity continues while the job runs.
- [ ] **GEN-08 — Improve without resetting the world.** Replacing a visual asset preserves entity IDs, owner, location, task, stored goods and history. Changes to gameplay rules/costs need separate validation; an art revision cannot grant resources or change a building's function silently.
- [ ] **LIVE-01 — Discover and deduplicate useful work.** While empires run, their missing building/unit capabilities or recorded visual defects create relevant jobs without a developer prompt. Resolved needs stop generating jobs; duplicate requests do not cause duplicate paid work.
- [ ] **LIVE-03 — Freeze a bounded job.** Save the need, empire, theme, role, site constraints, criteria version, source state, format and time/call/spending caps before dispatch. Wrong-role candidates fail; resuming cannot reset the three-candidate/twenty-minute limits.
- [ ] **LIVE-04 — Review actual renderer images.** Capture the exact candidate in at least three views covering silhouette, materials, ground contact and access. The independent reviewer receives those images and records per-check judgments. World integration requires actual Bevy scene evidence.
- [ ] **LIVE-05 — Demonstrate repair.** After the release is fixed, the auditor inserts an undisclosed blocked access, floating support or missing surface. The reviewer finds it in actual views; the creator repairs it within the original limits. Retain both candidates, matched before/after views and the previously failing functional check. Flawless routine drafts need no invented defect.
- [ ] **LIVE-06 — Separate creation and acceptance.** An independent check covers frozen appearance requirements, scale, files, format, collision, access, supported animation and function. Any mandatory failure prevents activation; the creator cannot edit the judge or approve itself.
- [ ] **LIVE-07 — Activate reviewed bytes only.** Link brief, attempts, images, critique, checks, usage/session metadata and receipt to exact candidate/evidence hashes. Both observers render the same approved files; changed/stale evidence invalidates approval.
- [ ] **LIVE-08 — Update a running city compatibly.** Stage an approved compatible asset in both views, then activate one manifest at an authoritative tick. Construction, resources, unit tasks and entity identity continue; both clients use matching collision/role properties.
- [ ] **LIVE-09 — Handle an unready viewer.** A slow or incompatible observer remains on a mutually valid version or receives an explicit reconnect/upgrade requirement before activation. Rejoining loads the active compatible manifest without missing geometry/collision.
- [ ] **LIVE-10 — Stop safely.** Generation failure, exhausted budget and cancellation each stop with a saved reason and the prior city/release intact. Late, duplicate or cancelled submissions cannot activate content or trigger further spending.
- [ ] **LIVE-11 — Roll back exactly.** After a detectable runtime failure, restore the prior approved compatible manifest within 60 real seconds. Both observers and a reconnecting view receive it without duplicated entities, changed ownership/resources or lost city history.
- [ ] **LIVE-13 — Reject invalid packages.** Separately reject unsupported format, missing dependency, invalid collision and post-review file changes before activation. Preserve the previous accepted release and the recorded rejection reason.

## Core ACs — persistence and presentation

- [ ] **WORLD-01 — Share one authoritative city state.** Two observer clients agree on accepted building/unit changes within one second under reference network conditions. Each camera is independent; neither viewer can issue empire orders. Observer-wide inspection is never fed into faction knowledge automatically.
- [ ] **WORLD-03 — Reconnect to the actual world.** Disconnect during construction, production, delivery and asset staging in separate tests. Rejoin restores current city state, resources, tasks and manifest, without recharging or completing an action twice.
- [ ] **WORLD-04 — Grow without viewers.** With active city orders and expansion plans, disconnect all observers for ten simulation minutes while the host runs. On return, all three cities have progressed under their actual rules, with recorded autonomous choices and outcomes.
- [ ] **WORLD-05 — Recover acknowledged actions.** Stop the host unexpectedly after acknowledged construction, unit production, delivery and asset activation. Restart restores each durable effect once, empire memory and the active manifest. Unacknowledged actions resolve explicitly; host downtime is paused simulation time.
- [ ] **UX-01 — Keep the observer view readable.** At 1920 × 1080, camera help, city/empire inspection, current work and optional notices remain usable without clipped or overlapping controls. The default experience centers the changing cities.
- [ ] **UX-02 — Explain blocked empire work.** Inspection shows the actual missing resource, power, access, rights or design prerequisite for a blocked order and its current status. Rejected proposals consume nothing and cannot appear complete.
- [ ] **UX-03 — Meet a visible art standard.** Actual city, building, civilian-unit and coast views use consistent original science-fiction shapes, scale, materials and lighting. Missing textures, debug placeholders in a claimed finished scene, floating structures and unreadable silhouettes fail visual review. Asset provenance is retained.
- [ ] **UX-04 — Explain agent decisions and content work.** Inspection shows each empire's current goal, chosen building/unit/site, concise reason, order status and actual result. The operator Studio separately exposes permitted observations, job versions, images, checks, usage, budget and release links; private evidence is not broadcast to empire minds.
- [ ] **UX-05 — Keep observation responsive.** Meet the frozen 20-minute reference load/frame targets in the audit protocol while flying between all three growing cities and activating compatible content. Record measured results; absence of a capture is blocked.

## Integrated milestone scenarios

- [ ] **FLOW-01 — Watch three cities emerge (D0).** From a fresh seeded minimal kit, run 20 simulation minutes through normal empire decisions. All three empires choose buildings/units and sites, pay their costs and develop the SET-01 city clusters. Observe from two cameras; retain timed wide/close views and decision → accepted order → completed function records. Manual placement, scripted decorative reveal and prebuilt snapshots do not prove growth.
- [ ] **FLOW-02 — Close both content paths (D0).** In the same integrated release, unmet needs commission one building and one civilian-unit design. Real renders and independent review drive at least one evidenced repair. Exact accepted packages unlock; empire decisions choose placement/production, pay and put both to use. Reconnect preserves city state and manifest. A Studio-only fixture or free mesh spawn is blocked for this scenario.
- [ ] **FLOW-04 — Sustain and repeat (D1).** In a 30-real-minute run, change a known resource need and show a different useful investment plus a second bounded content cycle. Include ten simulation minutes without observers, a five-minute decision-service outage, reconnect and one host restart. Reconcile resources, buildings, units, decisions and approved hashes with no unexplained reset, duplicate grant or unfinished action shown complete. No human rescue; retain failures and actual call/spend evidence.

## Optional polish — significant events and notifications

These are not D0/D1 gates. Deliver them only after the core city/content loop works.

- [ ] **EVENT-01 — Notify only real significant changes.** A completed landmark, accepted empire choice or event start produces one concise notice with empire, time, actual outcome and inspectable map target/history. Duplicate delivery/reconnect does not replay it as a new event. Routine ticks do not flood the view; notices never invent activity.
- [ ] **EVENT-02 — Respond to an observed event.** A recorded shortage, supply interruption or terrain event prompts a feasible city response within 90 seconds: different construction, reroute or relocation proposal. An empire without an observation/report cannot refer to it. Show the accepted action and visible result.
- [ ] **EVENT-03 — Keep a disruptive event recoverable.** If an event blocks a city's required route/site, preserve a feasible alternate supply, reroute or new valid site. The empire uses it through normal costs and placement rules; no free inventory or administrator rescue.

## Final stretch — talking to races

Only after D1. Camera mode remains the entire human presence. No walking, personal inventory, quests/rewards, military systems or human empire management is required. Full treaty economies are outside this stretch; basic grounded conversation is its finish line.

- [ ] **DIP-01 — Open the right race's leader.** From city inspection, open a leader panel with the selected race/empire identity, attitude and permitted known facts while the world stays visible. Switching leaders updates all fields; closing returns to camera observation.
- [ ] **DIP-02 — Talk naturally while cities run.** Ask about the race, its city, needs and a recent significant choice in ordinary language. Receive a relevant reply or clarification. Show waiting/cancel after five seconds and an explicit retryable failure; ongoing city work and camera controls remain responsive.
- [ ] **DIP-03 — Ground every factual reply.** Replies match that empire's permitted resources, goals and recorded observations; unknown facts are acknowledged as unknown. Preserve contact history across reconnect/restart without sharing it with unrelated empires.
- [ ] **DIP-14 — Keep speech within authority.** Requests for free goods, another empire's land or hidden knowledge cannot create world effects. A model response cannot bypass the same action validation used by autonomous empires.

If bounded agreements are attempted as part of this final stretch, both of these additional checks apply; omitting agreements does not block grounded conversation:

- [ ] **DIP-04 — Show exact proposed terms.** A supported city/logistics agreement displays parties, quantities/rights, sites, conditions and expiry before consent. Unsupported terms receive a clarification/refusal, not a fabricated promise.
- [ ] **DIP-05 — Confirm the exact version once.** Authority requires matching eligible consent and rechecks ownership/resources for the displayed offer version. Expired, declined, stale or merely discussed offers change nothing; duplicate acceptance has no repeated effect.

## Verification commands and evidence boundary

For the asset slice: `cargo build -p living-world-core`, `cargo test -p living-world-core`, `npm test`, then `npm run test:e2e` with Studio running. The last command is offline rehearsal with actual renderer captures, repair, activation, cancellation and rollback; it is not live generation/review proof. Separately demonstrate actual Gemini creation and independent image review with frozen spending limits, usage/session metadata and before/after evidence.

For the world slice, run the relevant workspace checks and normal Bevy scenarios from the [development guide](dev/README.md), recording the commands, exit codes and actual views for the touched criteria. Passing implementation tests does not check off these ACs. Missing live integration, renderer parity or provider evidence remains blocked. Public hosting, publication and additional paid testing require their own authorization.
