# Terrain integration work record

Owner: terrain chat / root. Base db64e4d. Primary EXP-01/09/10/11. Affected PLY-02/03/04, WORLD-01/03/05, GEN-06, UX-03/05, AUD-03.
Owned paths: dev/runtime/src/world.rs; game/src/protocol.rs, client.rs, main.rs; game/tests/terrain_network.rs; this record and terrain spec/plan.
Dependencies: terrain-core owns terrain v1 records/query/persistence; terrain-render owns native meshes and visual.rs. Existing bilal-gemini-host owns server.rs, runtime lib.rs and dependencies until handoff. User authorized cross-chat coordination. Friend's generated model work is separate; no asset format is modified here.
Checks: terrain seed/connectivity/seams/save tests, authority rejection/retry/restart, real network clients, native visual audit, workspace/Python regression checks. Full economy, fog, events and reference-device performance are unavailable and cannot be marked passed.
Progress: goal created, full README read, isolated worktree and narrow integration claim established. No implementation or AC passes yet.

Verification before changes: parallel workspace baseline failed three existing one-second Lua subprocess guards; serial baseline failed native_collection_operations_cannot_bypass_instruction_limits. Isolated rerun passed (0.24s). Logs retained in /private/tmp/argon-terrain-baseline*.log. No guard loosened. Seed CLI test failed on unsupported --seed, then both binary input tests passed after parser support; host plumbing awaits claimed-file handoff.

Ruling: preserve pre-terrain schema1 worlds as legacy arenas rather than reinterpret their saved coordinates on new land. New native saves use explicit terrain sidecars and seed; an explicit seed on an established legacy world fails with a new-save instruction. This preserves EXP-10/identity, at the cost of requiring a new named save to try terrain from an old arena.

Handoff: user-authorized direct-main coordination moves this stream to /Users/bilal/projects/argon-empires. Existing worktree edits are preserved as the candidate, not discarded. Core7c5984d is imported as explicit owned files; other main dirty files belong to Studio/build-authority/coordinator and must not be staged. Main commits use integration lock and exact owned paths.


Native checkpoint: saved schema2 terrain reference and atomic baked sidecar; legacy
schema1 coordinates remain unchanged. Protocol2 manifests and bounded chunk
requests hold readiness until the geometry fingerprint is verified. New native
worlds use saved terrain; --seed applies only to creation. Authoritative movement
checks triangles/water/slope and resolves acknowledged retries before revalidation.
Native renderer streams bounded chunks with exact query triangles, five blended
biomes, ocean, terrain-grounded decoration and independent cameras.

Evidence before publication: seven core terrain tests; four world terrain lifecycle
tests; real UDP two-client terrain/fingerprint/restart test; native renderer tests.
Full workspace run at base17e478e passed except existing activation network test
health startup deadline; exact same executable isolated retry passed in3.14s.
Failure and retry preserved in /private/tmp/argon-terrain-final-suite.log and
/private/tmp/argon-terrain-network-retry.log. No deadline loosened. Independent
review found stale terrain on manifest transition; lifecycle regression tests
reproduced it and the renderer owner repaired scene retirement/recentering.
Latest-main final checks recorded in /private/tmp/argon-terrain-publication-check.log.

Actual fixture render exists, but two real native-client captures and multi-seed
journeys remain pending. No full EXP/UX/AUD criterion marked passed. Paid city
placement, building collision and fog remain separate owner dependencies. After
this checkpoint G receives World/lib ownership for durable settlement authority.

Final current-main run: native lib20/20, network2/2 plus terrainnetwork1/1, World4/4 passed. Clippy rejected three decorative angle approximations6.28; replaced with exact TAU constant, no authority change. Original lint failure retained in /private/tmp/argon-terrain-publication-check-before-tau.log.
