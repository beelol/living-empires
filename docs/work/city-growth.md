# City growth stream

Owner: city chat 01a1748d-ea41-7d81-a580-8378a16b1565. Candidate base 661de330.
Primary ACs SET-01, SET-02, FAC-05. Affected GEN-01, GEN-06, GEN-07,
EXP-10, EVENT-01, WORLD-05. Read the entire README and AGENTS.
Owned paths: dev/runtime/src/city/, dev/runtime/tests/city.rs, this record,
docs/superpowers/plans/2026-10-25-city-growth.md. Claim bilal-city-growth
in isolated managed city-growth checkout. Main integration remains the target.

## Contracts and decisions

User explicitly directs continuing implementation without design approval loops.
Use existing Lua faction needs and host scheduler; no new terrain, scheduler,
world writer or demo. Lua ranks bounded settlement offsets; Rust asks G's exact
placement validator for each candidate. A proposal cannot grant ownership,
resources, approval or completed buildings. Only authority can accept it.

Terrain owner 01a17427-b259-78f0-83bc-5201a187c7e8 supplies baked v1 Terrain,
geometry fingerprint and world revision; sample/can_traverse do not imply rights.
Construction owner 01a171d8-149a-7613-bd7d-1eb488e82b22 supplies placement v1
FootprintRecipe/SiteRequest/PlacementContext -> opaque LegalSite and paid World
transaction. Terrain currently owns World/lib/protocol/server and hands those
paths to G. Brain owner 01a17437-6306-7ac2-a731-8b43d2e66ec7 supplies real
needs and approved recipe references. GM owns sole decision cadence. Visuals
01a17443-0241-72e2-b389-1543495cd34d consumes persisted buildings and routes.
Starter mapping/World hooks pending owner confirmation; do not invent live IDs.

Design choice: a connected entrance network around an open forecourt. Organic
uses asymmetrical lobes, cyborg uses paired terraces, robot uses ordered bays.
All buildings retain the validator's fixed -world-y entrance orientation. Bounds
and route clearance remain validator-controlled. Accepted buildings and routes
are never deleted by a layout refresh. Terrain changes invalidate pending site
proofs; fresh proposals must validate against the new revision. Exhaustion yields
an explicit blocked reason. Lua edits stage into a bounded fresh VM and promote
only a valid, bounded result; invalid edits retain the last good policy.

## Evidence ledger

No live city pass yet. Shared main contained foreign Cargo.lock, placement and
Foundry changes; preserved. Google Drive reference returned NOT_FOUND; web fetch
also inaccessible. Local README remains canonical. No paid calls made here.

## Checkpoint 1 verification

Five behavioral tests initially failed against the empty proposal boundary; all
five passed after implementation. Actual G placement then exposed the third
organic building's access dead end. Prioritize reachable forecourt bays before
rear terraces; the real-terrain test now places three successive structures for
each species, reserving exact footprint/entrance/approach bounds and rejecting
stale proofs. Geometry test seed20261025; all assets here are explicitly fixtures.

Independent reviewer found two important Lua defects: conditional founding-only
validation could promote a looping growth branch, and base setmetatable allowed
an uninterruptible finalizer during VM teardown. Both were reproduced first:
conditional regression failed, finalizer child timed out after3s and was killed.
Repair validates all six species/founded branches before promotion and removes
metatable APIs. Ten city/placement tests now pass, one isolated child probe is
intentionally ignored in the outer suite and is actually executed by its parent.
A pre-review full argon-dev run passed126tests; fresh combined post-fix run follows.

Terrain grants new game/src/city_visual.rs ownership. G's settlement::BuildingView
is the agreed renderer input; no invented local city. Gowns native authored
starter roster/costs and sole durable city state; helios=robot,tide=organic,
cinder=cyborg. User explicitly accepts simple native models now and Foundry later.
Ruling: forecourt-first bounded growth is the first connected increment; rear
terraces may remain blocked under v1 straight-approach geometry rather than
bypassing route checks. Arbitrary larger networks need an agreed route extension.

Post-review full runtime attempt failed the existing content test
`native_collection_operations_cannot_bypass_instruction_limits` at its one-second
subprocess deadline; isolated unchanged recheck failed too. Failure logs retained
/private/tmp/argon-city-{runtime-reviewed,content-recheck}.log. City-specific
10 tests and clippy pass after both review fixes. This is not a green full-suite
claim; foundation is notified. No guard or foreign content file was changed.
