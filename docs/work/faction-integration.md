# Faction-scoped appearance integration

Owner: Sol / faction-integration. Base dc45f943c77debbc2ca4e6157fdc39b4863fc808.
Primary reference ACs GEN-08, WORLD-05, LIVE-10; affected GEN-02, UX-04.
Owned paths: solo/server.mjs, solo/test/server.test.mjs, this record.

The user requested selection and assignment in First Landing, the existing separate single-player app. These are appearance groups, not native empire ownership. README v2 camera-only rules remain unchanged; no Bevy milestone pass is claimed. State owner faction-state defines solo save v2 migration and scoped command/request fields; faction-adapter owns exact-target installation and rollback; faction-ui owns selection and rendering. Foundry HTTP/package contracts stay unchanged; solo HTTP API v1 gains compatible fields and commands.

Server transactions preserve the identity of in-flight request records while committing newer faction intent. Superseded remote jobs are cancelled after the local cancellation is durable. Unknown remote cancellation remains visible and cannot install locally. Retry/reconcile cannot revive a stale faction revision or reassigned target. Public state includes the actual Foundry URL for history links. Regeneration retains the existing building's faction and assignment generation.

Checks pending combined integration: scoped API commands and restart, stale in-flight reference, all existing solo tests, actual browser area selection and generated faction isolation. Live results and commit will be recorded separately; implementation tests do not establish visual acceptance.

## Verified live result, 2026-10-25

Implementation published to main through a47f993bbd1d626f520ce4baad7a05cf2600da5b. Combined colony suite: 58/58 pass. Existing Foundry contracts and Rust verifier are unchanged; running verifier is /private/tmp/world-check-art-quality-b2f76c4, creation/repair gemini-4-argon, independent visual critique gemini-3.8-flash. Shared $500 cap and original ledger retained; allocated allowance is not billed spend.

Actual app: http://127.0.0.1:4320/, Foundry http://127.0.0.1:4318/. UI was operated at 1280px and observed at 880px: create Copper Coast, select two opposite map corners, preview highlighted targets, assign two western solar buildings. Target IDs faction-demo-west and faction-demo-northwest; eastern same-kind faction-demo-east and landing-core stayed in Landing. First style inference failed its existing 500-byte shapeLanguage bound; the paid response and blocked request remain in history. The correction adds explicit concise field budgets before a new inference, without truncating a response or weakening validation.

Live job job-basalt-copper-modular-1792968720919 used 4 creator/reviewer calls and 2 candidates with injectDefect false. Candidate 1: 38af3d7040c6762de39186cdb43a4f71c195d6f2faab77dc0a47b92926b6838c. Flash identified projecting cyan mounts where the frozen design required recessed wall reveals. Candidate 2: b3f48c1facfebd6e93599d89d04fffafb25706b835495bad438a3b3be9662b4f. Independent comparison says improved: cyan indicators sit in recessed reveals, while asymmetrical massing, copper ribs and plot clearance remain; no regressions reported. Three matching-camera renders per version are retained under .data/job-basalt-copper-modular-1792968720919/{1,2}-{0,1,2}.png. This is a live creative repair, not a fixture or injected defect.

The colony's typed verifier checked the approved package and all three exact image hashes, then installed the same accepted hash on exactly the two selected targets. At installation, the outside solar and core appearances were unchanged, building identities/positions and 95 alloy were preserved. .data/faction-live-audit.json and .data/faction-live-baseline.json retain the comparison. User subsequently requested a separate Landing update from the dashboard; those independently scoped requests were preserved and are not part of the Copper Coast isolation assertion.

A graceful restart onto canonical main retained exact building records, factions, accepted assets, history, alloy and live job receipts. .data/faction-restart-audit.json records this. The browser's in-progress user text was not reloaded. These observations verify the First Landing appearance-group feature only, not native Bevy empire ownership, autonomous faction decisions or the D0/D1 integrated milestone. Native village visual binding remains separately owned.
