# Saved faction areas and inherited design requests

Owner: faction-area-state. Base: `c2a445c8ff4b40d13d0124ce02196a177b6f6af8`. Primary references: GEN-01, GEN-02, GEN-08, WORLD-05. Affected: LIVE-10, UX-04. Owned paths: `solo/model.mjs`, `solo/test/factions.test.mjs`, this record.

Read the complete README contract v2. This remains the user's separate First Landing appearance-group exception; native Bevy empire ownership and observer controls are unchanged. No native-game milestone is asserted passed.

Shared interface agreed with lead, faction-area-adapter and faction-area-ui: optional saved `faction.area` extends solo schema 2 compatibly. Bounds are finite within ±1e6 and have positive width/depth; a positive monotonic order resolves overlaps to the latest saved area. Area assignment derives members authoritatively from building centers and rejects a stale supplied selection. Empty areas are valid and persist. Existing explicit selected-ID assignment remains supported.

Construction resolves the faction from its validated snapped site first, then the explicit/default selection. Ordinary regional or explicitly faction-selected construction queues an exact single-target `preferExistingDesign` request without cancelling or extending an in-flight job. Default API construction outside a region does not introduce paid generation. The adapter owns reuse of exact approved packages and rejection of stale/failed inheritance; no model assignment itself activates an asset.

`faction-assign` accepts `reuseDesign: true` to preserve current-revision family work while queuing verified inheritance. Existing custom creation requests stay attached to their original subjects and are retained for a same-membership reuse assignment; changed-faction custom subjects receive fresh generation. An explicit style update still increments the revision and queues every current member, including later construction, while retaining each previous accepted appearance.

Validation: `node --test solo/test/model.test.mjs solo/test/factions.test.mjs` — 23/23 passed. Added checks cover authoritative area membership, overlap precedence, stale selection rejection, unchanged in-flight targets, new member inclusion on explicit style changes, no accidental default paid work, custom subject retention, durable empty-area reopen, command retry idempotency and malformed-bound rejection. Actual browser selection and live accepted-package reuse are integration evidence owned by the lead; these tests alone do not prove visual quality.
