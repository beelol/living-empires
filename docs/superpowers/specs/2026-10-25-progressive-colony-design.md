# Progressive Foundry and autonomous browser colony

Date: 2026-10-25. Baseline: main `0679df026ea2faf41753d1075ea5382bace332f6`.
Status: design for owner approval; no feature implementation yet.

## Intent and scope

Extend the browser colony the owner is using at port 4320 and its Foundry at
4317. Show a useful first result quickly, then visible reviewer-driven versions.
Make expansion depend on finite metal, power, construction and paid civilian
bots. Bots can periodically commission Gemini decisions to expand the city.

This is the Solo browser prototype. README contract v2's camera-only Bevy
world, three empires, networking and integrated milestone gates are unchanged.
The owner explicitly requested this browser flow; its results must not be
reported as Bevy D0/D1 acceptance.

## Approaches and decisions

The owner selected **first approved version, then background improvement**, over
waiting for all refinement before installation. Unreviewed drafts are visible
in Foundry with their actual status; only independently approved packages enter
the city. The best approved incumbent survives rejected improvements.

A single unlimited pipeline would contradict the frozen three-candidate,
six-call, twenty-minute job contract. Use a durable refinement session containing
successive ordinary bounded jobs, each with an exact accepted baseline. The
session has no overall wall-clock deadline. It retains cumulative work, spend,
unknown outcomes and cancellation across child jobs and restarts; individual
calls still time out. Child jobs do not reset an unfinished job's limits.

The session shares the authorized $1,000 application spending ceiling with
generation, review, theme inference and city-decision calls. This is a ceiling,
not a target or an invoice. Unknown outcomes retain their reservation and pause
new dependent work. Cancellation or exhaustion stops further dispatch.

The builder may choose small module boundaries and exact timing constants within
the behaviors below. No new engine, renderer, public service or dependency is
required.

## Foundry behavior

- Save and display every returned candidate immediately; show pending capture,
  pending review, accepted, unchanged or rejected separately. Version numbering
  and history span the refinement session, with exact job/candidate/evidence IDs.
- Install the first approved compatible package into its original plot. Start a
  linked improvement job from that exact incumbent without repeating theme
  inference or losing the requested purpose.
- Ask the reviewer for concrete improvements to silhouette, composition and
  visible assembly; preserve strengths. Never manufacture a failed mandatory
  check just to generate another version. An unchanged/regressed candidate never
  replaces the incumbent.
- Continue useful improvement rounds without an overall deadline. Pause when the
  reviewer has no actionable improvement, repeated work cannot improve, an
  ordinary child job exhausts/fails, an unknown paid outcome needs reconciliation,
  the user stops it, or the shared allowance is exhausted. Show the real reason.
- Cancel is durable, invalidates late results and prevents a queued next round.
  A restart can resume only an unambiguous checkpoint; it cannot silently retry
  a dispatched call. Starting/reopening a session cannot reset its totals.
- Changes to appearance preserve entity, position, function, economic quote,
  work progress and bots. Geometry remains inside the fixed plot; collider and
  package checks remain authoritative.

## Economy and power

- Keep the existing metal/alloy currency and finite deposits. Existing extractors
  consume deposits and credit only that amount; no new currency is needed.
- Starting resources remain a limited bootstrap kit: 200 metal and the existing
  core's 8 power. This affords solar and extraction before larger investments.
- Validate available power before accepting any consuming building. Completed
  generators supply power; all accepted consumer orders reserve demand, including
  unfinished construction. Two concurrent orders cannot oversubscribe the same
  capacity. Rejection consumes no metal and creates no building.
- Solar consumes metal and time. Extraction needs power. Construction and bot
  production pause honestly during a shortage and resume remaining work.
- New generated creations require at least 250 metal, and generated bot factories
  at least 500, compared with current ordinary buildings at 35–70. The final
  bounded quote also reflects approved geometry: part count and total bounded
  volume. Use one deterministic, versioned formula, display its breakdown and
  freeze it on the first accepted design; later cosmetic improvements cannot
  alter costs or grant production.
- Placement reserves the minimum metal once. After approval, the remaining quote
  is paid once before usable construction completes; insufficient funds show an
  awaiting-materials state. Cancelling before construction refunds only the actual
  reservation, once. Existing paid buildings remain paid and are not charged again.
- Remove the fabrication lab's unlimited free-metal recycling. It becomes a
  metal-consuming civilian bot producer under the explicit economy rules update.
  Existing balances/deposits/buildings/receipts survive migration. Record the rules
  migration; do not pretend this is an appearance-only update.

## Bots and autonomous expansion

- Add a visible Bot Foundry construction choice with a fixed supported civilian
  production role and a generated appearance. A prompt or generated mesh cannot
  invent new functions. The ordinary fabrication lab can provide the starter
  production function; the generated factory is the expensive generated variant.
- A completed powered producer spends metal, takes production time, respects
  habitat capacity and an overall bounded bot count, and spawns at a valid exit.
  A blocked exit waits without duplicating or repeatedly charging the bot.
- Persist stable bot IDs, producer, position, route, task and production receipts.
  Bots independently choose reachable construction sites, travel there, and add
  actual bounded construction work. Idle bots travel between valid city sites.
  Blocked paths reroute or pause; they never walk through buildings or water.
- The original controllable robot remains usable. New civilian bots operate
  independently. Their useful construction contribution is visible and recorded.
- With autonomous growth enabled, a completed producer and its bots periodically
  ask Gemini to choose among current legal, affordable expansion options. Proposed
  options include power, extraction, housing and production, plus a bounded
  generated-building brief when useful. One decision is in flight at a time;
  use a default cadence around 90 real seconds and skip calls when no action is
  feasible or an equivalent plan is already pending.
- Gemini chooses intent and explains it. The server revalidates current metal,
  power, placement and capacity and commits an idempotent normal command. A stale,
  unaffordable or invented action is rejected without world effects. Provider
  failure pauses decisions while existing production and movement continue.
- A clear autonomous-growth toggle stops future decision calls. Display current
  goal, proposed/accepted/rejected decision, spending state and next useful work.
  Enable new autonomous work through the visible producer/growth flow; migration
  alone must not start an unseen paid decision loop on an existing save.

## Shared interfaces and ownership

Parent/integrator owns Solo server wiring and both UI surfaces. Economy worker
owns Solo model/save migration and economy tests. Bot worker owns new bot and
decision modules/tests, using the economy owner's normal command and movement
interfaces. Foundry worker owns server session orchestration and its modules/tests.
Every worker gets a separate worktree and exact named claims before edits.

Economy exports stable quote, power reservation and command validation helpers.
Bots receive the current world and legal options; Gemini cannot receive a mutation
callback. The parent wires periodic decisions outside synchronous simulation ticks.
Foundry owns the one shared dollar ledger and exposes idempotent decision requests
to the Solo server, so separate processes cannot race ledger files. The Foundry
worker and bot worker agree the route/module shape before implementation.

Additive Solo state gets a deliberate version and migration. Existing world IDs,
balances, coordinates, accepted asset hashes, deposits and receipt histories remain
unchanged. The state endpoint may add fields; existing Foundry job/package APIs
remain compatible. Do not change Rust asset acceptance or Bevy network schemas.

## Acceptance and evidence

Primary references: UX-04, GEN-08, SET-04, SET-06, SET-08. Affected: SET-03,
SET-05, SET-10, GEN-05, LIVE-03–07, LIVE-10, WORLD-03/05. These are behavior
boundaries; prototype evidence does not check off canonical world ACs.

Run deterministic tests for insufficient power/materials, concurrent reservations,
duplicate commands/refunds, finite deposits, cost preservation across art updates,
save migration/restart, paid bot production/cap/blocked exits, blocked movement,
stale model decisions, cancelled sessions and unknown paid outcomes. Fixtures are
rehearsal. Run existing npm, Solo and Rust checks against integrated main.

Then exercise normal browser controls: attempt an underpowered build; establish
power and extraction; commission an expensive generated Bot Foundry; observe two
or more actual captured versions and reviewer outcomes; install only approved
hashes; produce paid bots; observe movement and a normal validated autonomous
build. Reload and restart to verify no duplicate charges or lost state. Keep failed
outcomes, exact code/data revisions, snapshots, logs, screenshots and a clearly
labeled video. If live improvement or Gemini decisions fail, report them honestly.
