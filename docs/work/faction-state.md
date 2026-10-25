# First Landing visual faction state

Owner: faction-state (explicit successor to the preserved prior stream). Base: `dc45f943c77debbc2ca4e6157fdc39b4863fc808`.

Primary AC references: GEN-01, GEN-02, GEN-08, WORLD-05. Affected: LIVE-10, UX-04. Owned paths: `solo/model.mjs`, `solo/test/model.test.mjs`, `solo/test/factions.test.mjs`, this record.

This is the user-authorized First Landing appearance-group exception, not manual empire control in the native Bevy observer game. Membership changes never change building identity, function, cost, position, construction progress, resources or world ownership. No native-game criterion is asserted passed.

The agreed interface is solo save schema 2, compatible command additions on the existing HTTP API v1. The lead owns server transactions and cancellation; faction-adapter owns exact reviewed package installation, stale-scope checks and rollback; faction-ui owns area selection and per-building rendering. Foundry package and hash contracts remain unchanged.

Schema 1 saves migrate by pinning each currently inherited family asset to its existing building before disabling global appearance fallback. Saved factions carry an art direction, revision and inferred theme; building membership has a monotonic assignment number. `assetForBuilding` uses only the building's asset under schema 2.

Faction assignment queues exact selected members, grouped by kind, with a separate request per custom creation to preserve its subject. Style updates queue current faction members only. Both commands preserve visible accepted assets until replacements pass independent checks, retain superseded requests with reasons, and preserve in-flight request object identity in the model. Requests are explicit user work, so their `explicit: true` flag does not enable automatic world-wide generation. Stable operation receipts prevent retry duplication.

`creationRequest` accepts an existing `targetId`, `factionId` and `factionAssignment` for regeneration and rejects stale membership. Its request contains immutable target snapshots, faction revision, full user subject and art direction, plus a bounded dispatch prompt.

Validation: `node --test solo/test/model.test.mjs solo/test/factions.test.mjs` — 17/17 passed. New cases cover legacy migration, no global fallback, faction isolation, superseded in-flight identity, unchanged economy/assets, command idempotency, stale reassignment, normal construction membership, durable save/reopen and damaged-save rejection. These are implementation checks; actual browser selection, live creative quality and integrated acceptance remain the lead's pending evidence.
