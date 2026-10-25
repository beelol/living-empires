# Native terrain rendering stream

Owner: `/root/terrain_render`, claim `bilal-terrain-render`; base `db64e4d`.
Primary ACs: EXP-01, EXP-02, EXP-03. Affected: PLY-02, PLY-04, GEN-06, UX-03, UX-05, WORLD-01.
Owned paths: `game/src/terrain/`, `game/src/visual.rs`, this record.

Dependencies: `/root/terrain_core` owns terrain v1 (3072m extent, 8m samples, 128m chunks). `/root` owns client/network/authority and exposes `ClientState.terrain: Option<Terrain>`, validated geometry reconstructed from host chunks, with `terrain_info: Option<TerrainInfo>` to distinguish download from legacy worlds. Ready plus absent terrain_info means confirmed legacy. Core's `Terrain::info/sample/chunk` is the only source of terrain elevation. Row-major samples; triangles `[00,01,11]`, `[00,11,10]`. Normal computation uses global samples across chunk borders. No deposit or hidden-world knowledge is consumed.

Plan task 2: tests for topology/normals/bounded residency first; render biome-blended terrain, grounded procedural environment, ocean; preserve legacy arena, existing movement/status controls; ground commanders and ray-pick saved geometry. Build budgets and explicit handle eviction prevent unbounded retention.

Ruling: keep this per-stream record as the implementation ledger because parent owns the plan and independent review. Native screenshot evidence and full integration are parent responsibilities, not asserted by these tests.

Validation: in progress. No acceptance checkbox is passed by implementation tests.

## Handoff 2026-10-25

Ready for `/root` to integrate the owned files directly into `main`; no new worker commit was made, following the updated user direction. Base remains `db64e4d`. Renderer consumes core contract from `7c5984d` and parent-provided client fields. Parent owns final runtime export and network assembly.

Implemented: metre-space saved terrain rendering, ocean, five weighted ground palettes with generated mineral-grain albedo, global seam-consistent normals, batched rooted trunks/curved fronds/reeds/rocks and visible luminous seed stalks. Camera pans/rotates/zooms/tilts independently, recenters on the commander, stays above terrain, and uses exact triangle ray picking. Commander foot height follows the interpolated horizontal location. Confirmed legacy worlds retain their old arena. The overlay reports real biome, movement percentage, elevation and slope. F12 writes an actual native screenshot to `/tmp/argon-terrain-<unix>-<count>.png`.

Limits: two chunk builds per frame; 121 resident chunks maximum; at most five meshes per chunk, one ocean mesh, six shared materials and one texture. Retired meshes are explicitly removed. Decorative flora is presentation only, does not create terrain or deposits, and is not a new collision obstacle. No selection/construction feature was added.

Checks performed:
- RED: topology, plane normals at world edges, and bounded residency tests failed against empty initial implementations.
- GREEN: standalone geometry tests 3/3.
- Native Bevy 0.20 compile check passed using the actual renderer and actual core source in an external temporary harness. The harness substitutes only client-state/network shell fields, so this is not network integration validation.
- Seven native renderer tests passed (0.20s): topology/winding, edge normals, bounded residency, shared chunk positions/normals/colors, triangle-to-authority height agreement, vertical/oblique/edge ray hits and misses, continuous biome colors, deterministic finite decoration with valid indices and bounded geometry.
- Picking regression retained: initial oblique ray to (1536,1536) failed because epsilon-based grid stepping rounded back to the same cell. Integer-cell DDA fixed it; the same regression now passes.
- `git diff --check` passed. Parent must run combined native/workspace checks after integration.
- Actual native fixture captures: `/private/tmp/argon-terrain-preview-before.png` and `/private/tmp/argon-terrain-preview.png`. Final fixture uses seed 72913 at (1830,1500), luminous grove, 1600×1000 logical window/3200×2000 captured pixels. Final view visibly has connected trunks, curved fronds and protruding luminous tips. These are author-inspected fixture images, not independent full-game acceptance evidence.
- Initial restricted preview launch failed to find a GPU; rerun with Mac GPU access captured successfully. Fixture asset-watcher warnings reflect its absent external assets directory; renderer assets are generated in memory. Native linker warned about a large debug unwind table.

Outstanding: parent integration compilation with real client state; three-seed/two-client views and separate independent visual judgment; actual reference-load performance. No README criterion is checked off. Presentation owner may tune lighting/camera and use these stable fields: ViewState focus/yaw/pitch/distance (plus centered and existing input bookkeeping).
