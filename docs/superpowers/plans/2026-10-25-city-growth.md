# City Growth Implementation Plan

> Execute inline with superpowers:executing-plans and test-driven-development.

**Goal:** Three coherent navigable settlements grow through paid authority
commands on the same terrain, retaining cities across terrain and rule changes.
**Architecture:** Lua ranks bounded layout offsets; Rust checks each through the
existing placement validator. World persists the exact accepted pose, style,
route and recipe and publishes the same state to Bevy clients.
**Tech Stack:** Rust, mlua, existing Bevy/Lightyear and durable World.
**Spec:** User city-generation request and docs/work/city-growth.md.

## Global constraints

No duplicate terrain generator, world authority, scheduler or demo. No free
buildings, manufactured approvals, credential expansion or paid service creation.
Only owned paths are edited. Integrate checked small increments to origin/main.

## Review focus

Pending sites must not survive a terrain revision as legal. Previously accepted
buildings must not move on reload. Route reservations must protect existing
entrances. Malformed/looping Lua must preserve last good behavior. Candidate
exhaustion must report the actual block without unbounded search.

### Task 1: Bounded racial layout and access network

Files: city/mod.rs, city/layout.lua, tests/city.rs.
Consumes G placement v1 and terrain v1. Produces CityPlanner::propose with an
opaque validated LegalSite and route geometry, or an explicit blocked result.
- [ ] Write failure cases for three distinct bounded layouts, valid grounded
sites, occupied entrance protection, blocked terrain and bounded exhaustion.
- [ ] Observe failing assertions; implement minimal layout policy and validator.
- [ ] Run cargo test -p argon-dev --test city and targeted clippy.
- [ ] Coordinate public export and World consumer, verify combined checks,
commit explicit paths and push normal main under integration lock.

### Task 2: Paid persistent city + Bevy display

G owns the single World transaction and public snapshot fields; terrain owns
protocol handoff. City supplies route/style/layout version and next site.
- [ ] Wire on G's existing host boundary after handoff; no second scheduler.
- [ ] Run actual host/client, capture settlement views and authoritative records.
- [ ] Verify same buildings and navigation after reconnect/restart.

### Task 3: Incremental growth, terrain changes, Lua reload

- [ ] Write failures for revision invalidation, obstacle addition and invalid Lua.
- [ ] Revalidate pending sites on current terrain, retain accepted city history,
report blocked expansion, promote good staged layout at decision boundary.
- [ ] Exercise paid expansion and valid/invalid reload in running client.
- [ ] Record exact revision, images, state/cost changes and remaining gates.
