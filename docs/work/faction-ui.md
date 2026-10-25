# Faction appearance UI

Base: dc45f943c77debbc2ca4e6157fdc39b4863fc808. Successor of the preserved
faction-ui claim; this stream owns solo/public/app.mjs, scene.mjs, index.html,
style.css and this record. Primary ACs GEN-01, GEN-08, UX-01, UX-04; affected
GEN-02, WORLD-05. No native-world AC is marked passed.

This implements the user's explicit First Landing appearance-group request. The
README camera-only native Bevy contract remains separate: assigning an appearance
faction never changes ownership, entity identity, economics or building function.

Dependencies: faction-state owns world schema v2, faction commands and exported
assetForBuilding. faction-adapter owns scoped requests and exact accepted packages.
Lead owns server transaction/persistence and runtime foundryURL. Interface agreed
in /private/tmp/argon-faction-interface.md. No Foundry package, route or status
contract changed by this UI.

The map has explicit two-corner selection with a visible rectangle and highlighted
building footprints. Single-building inspection also opens assignment. A compact
Factions panel creates an appearance group, selects its members, updates only its
art direction and links every recorded design request to Studio renders/critique.
Draft text persists across 500 ms polls. Build and prospect commands carry the
chosen faction. Renderer and inspector use assetForBuilding instead of global kind
fallback. Faction rings are separate from accepted mesh materials; rollback is
scoped to the selected building's faction. Creation engine controls are collapsed
under Advanced.

Checks: node --check solo/public/app.mjs and scene.mjs passed. Actual browser area
selection, faction isolation and save/reopen are owned by lead integration and
remain not_run at this handoff. UI syntax checks do not prove artistic quality or
native-world faction behavior.
