# Faction-scoped appearance integration

Owner: Sol / faction-integration. Base dc45f943c77debbc2ca4e6157fdc39b4863fc808.
Primary reference ACs GEN-08, WORLD-05, LIVE-10; affected GEN-02, UX-04.
Owned paths: solo/server.mjs, solo/test/server.test.mjs, this record.

The user requested selection and assignment in First Landing, the existing separate single-player app. These are appearance groups, not native empire ownership. README v2 camera-only rules remain unchanged; no Bevy milestone pass is claimed. State owner faction-state defines solo save v2 migration and scoped command/request fields; faction-adapter owns exact-target installation and rollback; faction-ui owns selection and rendering. Foundry HTTP/package contracts stay unchanged; solo HTTP API v1 gains compatible fields and commands.

Server transactions preserve the identity of in-flight request records while committing newer faction intent. Superseded remote jobs are cancelled after the local cancellation is durable. Unknown remote cancellation remains visible and cannot install locally. Retry/reconcile cannot revive a stale faction revision or reassigned target. Public state includes the actual Foundry URL for history links. Regeneration retains the existing building's faction and assignment generation.

Checks pending combined integration: scoped API commands and restart, stale in-flight reference, all existing solo tests, actual browser area selection and generated faction isolation. Live results and commit will be recorded separately; implementation tests do not establish visual acceptance.
