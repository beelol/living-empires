# Product
<!-- impeccable:product-schema 1 -->
## Platform
web
## Stack
Delegated by the owner: separate browser app, Three.js, local Node authority, versioned JSON/HTTP integration. No multiplayer dependencies.
## Users and purpose
One player controls a robot and colonizes by clicking terrain and placing buildings. Asset Foundry progressively improves the actual city's appearance.
## Constraints
One-hour implementation budget on 2026-10-25. Separate app/worktree. Preserve other streams. No shared runtime/save/protocol changes. Existing v2 camera-only/three-empire/two-observer acceptance does not apply to this explicitly authorized alternate app; it does not establish v2 passes.
## Operating context
Local browser and loopback server; durable single-player save. Pulkit's existing Foundry HTTP service owns generation and independent acceptance. This app owns construction and visual activation only after verifying exact accepted packages. Fixture work is labeled rehearsal. Live paid calls require explicit operator configuration and Foundry's own limits.
## Product principles
Click actions have visible costs and construction time. Rejected actions change nothing. Upgrades preserve building identity, location, function and resources. Missing Foundry never stops colony play. Camera pan/zoom remain separate from robot movement.
## Defaults delegated to builder
Automatic sequential improvement of built building families; default fixture rehearsal, explicit live mode configuration. An authored starter kit is playable before any generation. Progress pauses while the local app server is closed.
