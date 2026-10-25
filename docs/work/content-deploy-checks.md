# Content deployment checks

Owner: Bilal / content-deploy-checks chat. Base: 347c8608d13a730d37fab3d7b82d07dfd513ae16.
Primary AC: LIVE-13. Affected: LIVE-08/09, WORLD-01/03/05, AUD-03.
Owned paths: dev/runtime/src/content.rs, dev/runtime/tests/content.rs, dev/smoke.py, this record.

Small delivery: reproduce and fix distinct Lua filenames being falsely rejected as ambiguous; exercise real commit deployment and two-client authority/reconnect checks.
Dependencies: gemini-host owns runner.py/server.rs and env-file forwarding; terrain-integration owns native client/protocol/world; terrain-core owns terrain records; terrain-render owns visual.rs. Preserve their current version 1 interfaces, saved-world format, deployment gates and all paths. No shared contract change is proposed. The local claim registry and each active stream record were inspected.

Checks: failing content regression before implementation; focused and workspace Rust tests; Python deployment suite; real supervisor smoke with isolated temporary remote. No full-game AC pass is claimed: actual two-device/reference-network evidence and approved asset release remain unavailable.

Plan: reproduce repeated-extension module-name collisions, strip only the final extension, verify content loading/reload/movement and rejection bounds, then run deployment and UDP checks. Integrate only explicit owned paths through the integration lock after refreshing main.

Initial hypothesis rejected: table.move with omitted destination works in the current Lua version. Its exploratory test passed and was removed; no wrapper change was made.

Verification (2026-10-25):
- Reproduced regression before fix: ambiguous Lua module: speed.lua.lua.
- One-line strip_suffix fix; 15/15 focused content tests passed.
- Full Rust workspace: 71 tests passed with one test thread; actual UDP authority/reconnect/crash checks included. Existing Darwin compact-unwind linker warning retained.
- Real isolated Git/host smoke: all eight checks passed. Warm initial build 43.652 s; local Lua module activation 0.630 s without host restart; native rebuild 6.753 s; remote Lua commit 0.839 s at 300 ms test fetch interval. Raw run: /private/tmp/argon-content-deploy-smoke.json. These are observations under concurrent build load, not latency guarantees.
- Independent read-only patch review found no concrete issues.
- Initial full Python suite: 38 tests, 5 startup errors: test_activation_marker_failure_restores_release_metadata_and_world, test_active_revision_cannot_rewind, test_host_preserves_explicit_env_file, test_live_candidate_cannot_accept_actions_before_health_promotion, test_malformed_validation_response_keeps_previous_host. Failures were fake-host build/validation startup timeouts while Rust compiled; no timeout was loosened. Full serial rerun pending, retained log /private/tmp/argon-content-python-rerun.log.
- Main checkout currently has another owner's untracked dev/studio/, docs/work/studio-bridge.md and spec/plan. Studio record identifies bilal-studio-bridge as owner. These paths are outside this claim and will not be staged or removed. Integration must respect the shared main slot and lock.

Final checks: formatting and workspace clippy passed. The second full Python rerun again had 37/38 passing: test_activation_marker_failure_restores_release_metadata_and_world timed out starting its initial fake build. Its isolated rerun passed without changing deadlines (5.804 s); the earlier four other cases passed in subsequent full runs. Full-suite timing reliability remains unresolved and is explicitly not reported green. Existing Gemini-host verification independently records the same startup timing limitation. This patch changes neither runner nor its fixture deadlines.

Integration direction: coordinator messages report the user's newer direct-main/shared-checkout instruction. Preserve named Studio, build-authority and faction-brain work; serialize the slot and stage only these four owned paths. ContentRuntime public interface, rules schema 1, world/protocol schema and active pack format remain unchanged. The fix accepts distinct names according to the existing one-extension module convention; no current checked-in pack uses the previously collapsed repeated suffix.
