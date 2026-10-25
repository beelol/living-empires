# Authority-owned placement v1

Owner: foundation thread. Paths: dev/runtime/src/placement.rs,
dev/runtime/tests/placement.rs, this record. Terrain owner explicitly handed off
these new paths; World/server/protocol stay terrain-owned until its checkpoint.
Primary SET-01/02; affected GEN-06, PLY-02/03, SET-04/13, WORLD-02/05.
Full README read; no completed AC claimed.

Contract agreed with terrain and brain: LegalSite is opaque, server-issued, bound
to actor, exact server recipe/fingerprint, terrain fingerprint and World revision.
It is not deserializable from a player proposal. Revalidate with the authoritative
world immediately before the same atomic placement/debit/outbox transaction.
Hash syntax is not content approval; caller must resolve independently approved
roster recipes. Terrain's sample/can_traverse are geometry, not permission.

First supported building geometry is an axis-aligned footprint with a south-facing
entrance corridor. Inspect every intersected baked terrain triangle, clipping to
the requested bounds so interior water/ridges cannot hide behind safe corners.
Require land, slope limit and supported height variation. Check corridor width,
overlap with existing/reserved obstacles, ownership and an accessible approach
from the authoritative builder/anchor. This v1 accepts conservative straight
approaches, not arbitrary routing; full navigation remains a separate dependency.
New core claims have the README's 80-metre radius and cannot overlap any claim.
Ordinary buildings must fit inside an owned claim; foreign treaty construction
rights are unavailable and fail closed until their authoritative contract exists.

Test plan: real generated terrain + owned/unowned claims, overlapping cores,
obstacle/entrance rejection, stale revision and malformed inputs. Geometry kernel
tests include a ridge/water pocket strictly inside otherwise safe corners and
clipping a steep neighboring triangle outside the actual footprint.
Next integration must add real inventories/claims/entities in World atomically;
no second save writer, fabricated site token, free structure or fixture AC pass.

Verification: five placement tests passed, including the two triangle-kernel
regressions, and clippy with warnings denied passed. An immutable 661de33 snapshot
with the normalized dependency lock passed all 16 placement, construction and
terrain tests. The final approach_bounds getter was then checked by repeating
placement tests and clippy. City consumes that exact route reservation instead
of rebuilding geometry. No live building or full-game AC pass is claimed.

Next: terrain owner hands over World/lib after native checkpoint; foundation owns
atomic paid placement and durable outbox integration, Studio owns verified Foundry
candidate delivery, and city owns bounded Lua layout proposals. Approved native
recipes and authority-owned faction identity mapping remain required dependencies.
