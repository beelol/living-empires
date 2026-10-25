# Finish-pair adapter port

Owner: `finish-pair-adapter`. Base: `a47f993bbd1d626f520ce4baad7a05cf2600da5b`. Primary ACs: LIVE-03, LIVE-10. Affected ACs: WORLD-05, UX-04, GEN-08. Owned paths: `scripts/dev.mjs`, `solo/launch-config.mjs`, `solo/foundry.mjs`, `solo/server.mjs`, `solo/public/app.mjs`, the three listed Solo test files, `solo/README.md`, and this record.

Scope: port the completion and pairing parts of `codex/foundry-fast-iteration` onto current main while retaining the faction-scoped asset and rollback work already in `a47f993`. This adapter port does not change world economy or unit production. No live services are started or restarted and no provider calls are made by this stream.

Dependencies and interfaces:

- `provider_diagnosis` / `finish-foundry-core` provides `/api/config` protocol and source identity: `service`, `sourceRoot`, `protocolVersion`, `packageVersion`, `rustBinarySha256`, `roleModels`, and `capabilities` (`nativeProvider`, `pipelineVersion`, `providerFormat`, `maxAttempts`, `maxProviderCalls`, `noDeadline`, `styleCompletion`, `sourceSha256`). New requests use the style-completion profile with 30 candidate / 60 counted-call maxima and no overall deadline; historical contracts retain their frozen limits.
- The same stream provides `/api/v1/health` inputs through the Solo server: app source root/hash/revision, API/save versions, and configured Foundry URL. Launcher reuse requires the matching checkout and service pair.
- Existing renderer/package integration remains authoritative: candidates are displayed as unapproved previews; package verification, three distinct image hashes, faction revision/assignment guards, and exact accepted bytes remain unchanged.
- Session dollar allowance remains separate from job call limits. The launcher does not enable sponsored credits or change the allowance.

Changes: terminal jobs are visible and can be explicitly regenerated for the same funded object while preserving prior history. Theme-inference failures update the intended building's status; a queued or pre-theme request can be cancelled without inventing a remote receipt. An unknown dispatch remains unretried until reconciled and is explained in the UI. The launcher starts/checks a private same-checkout Foundry binary and verifies both health/config handshakes before reuse. Candidate images are shown only as unapproved previews until exact package verification succeeds.

Checks: `node --test solo/test/launch-config.test.mjs`, `node --test solo/test/foundry.test.mjs`, `node --test solo/test/server.test.mjs`, `node --test solo/test/*.test.mjs`, `npm test`, `node --check scripts/dev.mjs`, `node --check solo/server.mjs`, and `python3 dev/agent.py check --id finish-pair-adapter --base a47f993bbd1d626f520ce4baad7a05cf2600da5b`. Local HTTP tests use only isolated ephemeral test servers. These tests do not establish LIVE or integrated game acceptance.

Evidence and unresolved scope will be appended after the reconciled checks. No live generation, release activation, or game milestone pass is claimed.
