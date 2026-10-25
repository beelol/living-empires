# Faction area UI follow-up

Base c2a445c8ff4b40d13d0124ce02196a177b6f6af8. Primary GEN-01, GEN-08, UX-01,
UX-04; affected GEN-02, WORLD-05. Owned solo/public/app.mjs, scene.mjs,
index.html, style.css and this record. Full README reread; native Bevy camera-only
contract remains separate from the user's explicit First Landing appearance-group
request. No ownership, economy or native-world contract is changed here.

Dependencies agreed with faction-area-state: optional faction.area bounds/order,
factionAt helper, faction-assign area/reuseDesign, and build result factionId.
Faction-area-adapter owns exact approved-design reuse and independent evidence.
Lead owns server integration, live browser verification and publication. Shared
interface recorded in /private/tmp/argon-faction-area-interface.md.

The drawer now keeps its faction header and primary Update faction action visible
while the middle scrolls. Direction is first, area saving is separate, and history
starts collapsed. Opening it hides the build shelf and inspector to free the map.
Typing preserves a local draft only; Update faction remains the explicit action.
New area selection supports dragging or two corners, passes exact world bounds,
and sends reuseDesign:true. Saved areas remain visible as dashed map outlines.
Placement previews identify the authoritative area's faction and cost; the accepted
build response determines the completion notice's faction.

Per-building progress compares the installed request's faction revision, exact
assignment and content hash to the currently visible asset. Last-good appearances
remain identified during newer queued/stopped work; old approved designs are not
counted as the latest direction. Member details retain visible version hashes and
job links remain stable during polling.

Checks: both JS files pass node --check; all referenced static DOM IDs exist with
no duplicates. Actual 880x850 and 1280x720 usability, saved-region inheritance,
accepted design reuse and persistence remain lead integration evidence, not_run
at this handoff. No paid calls were made by this stream.

Live follow-up: lead verified drawer controls at 880x850 and 1280x720, saved-area
assignment and new construction inheriting its area faction despite a different
selected fallback. The dashed one-pixel green saved outline was not clearly visible
in actual screenshots. Replaced only that overlay with 0.24 m violet border meshes,
fog and depth tests disabled; accepted asset geometry/materials remain untouched.
The area note now explains that resizing retains existing assigned buildings.
Both JS syntax checks and owned diff validation pass; final border visibility
remains lead's browser verification.
