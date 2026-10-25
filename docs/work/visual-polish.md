# Visual polish stream

Owner: beauty chat 01a17443-0241-72e2-b389-1543495cd34d; claim bilal-visual-polish.
Deadline: 2026-10-25 20:19 UTC (two hours from goal start).
Primary: UX-01, UX-03, UX-05. Affected: EXP-01/02/03, PLY-04, GEN-06.
Owned paths: game/src/presentation.rs; game/assets/shaders/landscape.wgsl; this record; docs/superpowers/plans/2026-10-25-visual-polish.md.
Dependencies: bilal-terrain-render owns visual.rs and terrain/, including Terrain v1 mesh geometry, ground picking and residency. bilal-terrain-integration owns ClientState and host transfer. Need explicit handoff/hook before integration. Coordination message was denied pending direct user authorization; no shared files edited.

## Design and decisions

Refine existing native Bevy presentation to NASA-inspired mission instruments: graphite panels, warm ceramic text, mineral teal status accents, amber warnings. Preserve broad world visibility. Replace the debug text wall with compact connection header, selected commander coordinates, movement feedback and quiet control legend. Do not invent inventories or health absent from authoritative state. A performance display reports measured client frames, not the full UX-05 audit.

Use built-in physically based lighting/fog and an inexpensive analytic water shader; no volumetric fog, full-screen ray marching or per-frame terrain rebuilds. Keep deterministic procedural scenery from terrain stream, same authoritative elevations and triangle geometry. Quality toggle must disable optional costs. Actual Bevy captures are required, including load/error states. Google reference document unavailable (Drive 404); NASA interpretation follows user's words and README palette.

Checks: compile/clippy, focused UI/status behavior tests, bounded frame samples, actual native screenshots and independent review. Existing game acceptance checkboxes remain untouched. Full UX-05 two-M2/reference-load audit is unavailable and must remain blocked.

Integration and evidence: pending.

18:39 UTC update: initial presentation compiles against disposable base harness. Terrain owner agreed to integrate hook only after combined compile/view. New module consumes stable terrain_info/terrain fields, preserves download/biome/movement information, uses embedded OFL Public Sans (Google Fonts source with bundled license). Embedded landscape.wesl offers normal-only analytic water; F2 disables shadows and ripples. F1 photo, F3 frame p95, F12 capture remains renderer-owned. No app launcher ownership: foundation thread owns native rebuild/launch helper. Client UI/shader changes require a native binary rebuild; they are not Lua hot reload or approved LIVE activation.

## Frozen handoff 19:24 UTC

Terrain parent accepts ownership of the presentation candidate and integration hooks. Source files: game/src/presentation.rs; game/assets/fonts/PublicSans.ttf; game/assets/fonts/OFL.txt; game/assets/shaders/landscape.wesl; this work record and plan. Minimal hook/material patch: /private/tmp/argon-visual-integration.patch, applies to terrain-render handoff and /private/tmp/argon-terrain-final. It preserves camera pitch/distance defaults, picking, streaming and geometry.

Evidence: /private/tmp/argon-visual-{preview,coast,grove}.png actual native fixture renders. /private/tmp/argon-visual-tests.log: 10 passed, including real-clock 1.2-second stall and 600-sample cap. Independent reviewer found no clipping or visible seams in supplied views; sole must-fix virtual-clock telemetry was corrected to Time<Real> and regression tested. Dense grove sample at1920x1080:491 frames after10s warmup, p95 25.10ms, max821.71ms; tiny fixture sample with no full load, not UX-05. Grove fixture uses camera180m/pitch0.48; these view settings are not in integration patch. Shader rendered without compilation errors. Native client integration/rebuild and two-view network evidence pending parent.

Opus kit found at .local/terrain-kit/HANDOFF.md: unpushed/uncompiled conflicting client generator must be excluded. Pure decorative prop geometry is suitable next bounded port to authoritative terrain. No second generator will be introduced.

## Private native integration candidate (terrain renderer handoff)

Candidate: `/private/tmp/argon-terrain-polished`, archived from `main` revision `9e1e37b`; publication remains parent-owned. Imported the frozen presentation module, `game/assets/fonts/PublicSans.ttf` plus `OFL.txt`, and `game/assets/shaders/landscape.wesl`. The two integration hook files are `game/src/visual.rs` and `game/src/terrain/mod.rs`. No camera/height/picking/streaming behavior, client/server/protocol, city, or authoritative-world file was changed. Manifest replacement cleanup and recentering from the terrain checkpoint remain present. The native transition test fixture now initializes the water material asset store.

Native default-feature build passed using a dedicated cloned-and-cleaned target. Executable: `/private/tmp/argon-terrain-polish-target/debug/empires`; SHA-256 `a7571ee47ac42b00b2539e53c8bcd43f47fdc6f424d62d7fe5f592cb7cec510e`. Font and water shader are embedded; the executable does not require loose copies of those files. Debug linker emitted the existing large unwind-table warning.

Focused native tests: `cargo test -p living-empires --lib --offline client::visual` passed 12/12, including the real-clock 1.2-second stall/600-sample limit, Unicode truncation, mesh/authority agreement, picking, and native→downloading→replacement / native→legacy lifecycle cases. Actual two-client native capture and independent visual judgments remain parent-owned and pending; this candidate does not independently establish UX-03 or UX-05 acceptance.


Terrain parent integration checkpoint: isolated terrain-audit checkout atc685fc6.
Minimal HUD/water hooks preserve geometry, picking, camera and manifest lifecycle.
Native build passed;12focused presentation/terrain tests and strict lib+bin clippy
passed. Three original test failures and review fixes remain in upstream evidence;
no full UX-05 claim. Final native SHA256
e39857ead8c601d82ddc587094a0a3a8db6e9847984926aa7db9ff51102eba87.
Logs /private/tmp/terrain-polish-{build,tests,clippy}.log. Existing fixture renders
show real shader output; fresh network-window inspection pending after CUA timeout.
Shader/UI owner receives presentation/assets back after publication; native
observer input conversion remains terrain owner, separately from UI refinement.
