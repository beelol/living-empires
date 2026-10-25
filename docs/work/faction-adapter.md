# Visual faction Foundry adapter

Owner: faction-adapter, successor to preserved claim in /private/tmp/argon-faction-adapter. Base dc45f943c77debbc2ca4e6157fdc39b4863fc808. Primary AC subjects GEN-02, GEN-08, LIVE-07, LIVE-10; affected LIVE-11, WORLD-05. Owned solo/foundry.mjs, solo/test/foundry.test.mjs, this record.

This is the explicitly requested First Landing appearance-group feature, not manual empire authority in the README v2 native game. Full README and workflow read. Assets cannot change building identity, economy, position, function or ownership. No native-world AC pass is asserted.

Dependencies agreed with faction-state: solo save v2 factions, monotonically changing faction revisions and building assignment counters, exact request targets, explicit user queues independent of the automatic toggle. Lead owns server in-place request transactions and remote cancellation. faction-ui consumes per-building assets and scoped history. Foundry HTTP API v1 and existing Rust package format/verifier/evidence checks are unchanged.

Each job freezes current faction and target identities. Installation rechecks revision and assignment before and after every asynchronous fetch/verification step, including image downloads. Late superseded dispatch results retain the returned job ID and request remote cancellation. A dispatch with no trustworthy response remains unknown and is never automatically repeated. One inferred style is cached per faction revision and copied into each job. Automatic discovery groups by faction and kind. Explicit user queues run while automatic discovery is paused.

Activation writes per-building assets only, with job IDs and per-target rollback records. It never writes the old global kind lookup. Rollback restores only still-matching members currently showing that accepted version, retains its historical entry, and cannot touch reassigned buildings. Legacy records without target identities remain preserved and are not eligible for a broad inferred rollback.

Checks: node --check solo/foundry.mjs; node --test solo/test/foundry.test.mjs: 21/21 pass. These are mocked provider/package rehearsals, including mismatch/evidence-integrity rejection, uncertain dispatch, cancellation, revision races, reassignment races, exact scope, rollback and shared style inference. No paid call or live renderer claim in this worker; integrated UI/live checks belong to lead.
