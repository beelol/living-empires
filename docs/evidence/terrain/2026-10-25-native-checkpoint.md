# Terrain evidence checkpoint — 2026-10-25

Owner: terrain root. Primary EXP-01–03/09–11; affected PLY-01–04,
SET-01–03, GEN-06, WORLD-01/03/05, UX-01/03/05, AUD-01–04.
This record is evidence of incremental implementation, not a full AC pass.

## Revisions and checks

- Core saved geometry published5a07994; native authority/transport/renderer
  published757ee07. Saved geometry seed72913 fingerprint
  b6eef4a5a785cd58f247f0db920691830950b3df0e8ec3e952bf6a9fb3172525.
- Presentation candidate36e99cf based on city baselinec685fc6 passed25 native
  library tests and strict library/binary clippy. It remains a candidate until
  its main integration revision is recorded; no implicit publication claim.
- Native observer input follow-up: two native regression failures before fix,
  one legacy test already green; after fix28/28 native library tests and strict
  clippy passed. Actual Bevy systems prove native WASD pans without world moves,
  stale local destinations are discarded, and legacy controls remain usable.
- Independent code review by terrain_review: no actionable critical/important
  finding; existing pending network actions remain untouched during reconnect.
- Prior complete workspace run retained one activation-network health-startup
  failure. The same executable passed its isolated retry in3.14s. Final native
  terrain publication checks passed both network tests, the real two-client
  terrain/restart test, and four World terrain lifecycle tests. No guard widened.

## Actual native evidence

These are local evidence files, not assets to ship or publicly hosted artifacts.
The first two use the same isolated authority on127.0.0.1:4017. No screenshots
were generated or edited. The polished view used a behavior-equivalent binary
before a syntax-only lint fix; its SHA256 is
 a7571ee47ac42b00b2539e53c8bcd43f47fdc6f424d62d7fe5f592cb7cec510e.
The corresponding frozen final binary SHA256 is
 e39857ead8c601d82ddc587094a0a3a8db6e9847984926aa7db9ff51102eba87.
Neither proves the later city server or new observer controls were running.

| Local capture | SHA256 | Scope |
| --- | --- | --- |
| `/private/tmp/argon-terrain-1792958093-0.png` | `ed0b9d380cd8f975e5a35bbac87617f66f25dddf7e587515edcf9dac0208b6a3` | Native client A, commander3, original HUD; shared host4017 seed72913. |
| `/private/tmp/argon-terrain-1792958650-0.png` | `3825fa053fafc0d0ae3439d519cc23b836fd8669d0eff0a3bbda184e03a0bedf` | Native client B, commander4, separately rotated camera; A remained unchanged on reinspection. |
| `/private/tmp/argon-terrain-1792959861-0.png` | `5b88e8ead0c5ae49d52e60f321bd171d969091ede8e19d92d912a93a1d441d60` | Polished HUD, connected client5; pre-observer-control presentation build. |

Independent terrain_review inspected the polished image: no visible ground gaps,
tile seams, missing surfaces, detached scenery or clipped HUD. Known commander
labels require the visual owner's observer-language follow-up. One2560×1600
viewpoint cannot prove coastline quality, all biome transitions, motion or load.

## Requirements still open

| Requirement | Evidence now | Remaining evidence/dependency |
| --- | --- | --- |
| At least4km² connected land and five reachable biomes | Core tests independently selected928751/44019/8170321:5.880/5.881/5.931km²; authoritative traversal flood-fill | Actual native journeys across all three seeds, independent visual acceptance |
| Stable saved geography and deposits | Baked geometry fingerprint, corruption/missing-sidecar rejection, save/reload/depletion tests | Normal gameplay mining and construction histories through restart, owned by authority/economy streams |
| Render/query collision agreement | Exact triangles, shared-edge normals and ray tests; movement rejection/retry tests | Actual building entrance/unit traversal on both current clients, owned with placement/city streams |
| Bounded streaming | Chunk residency/cap and lifecycle retirement tests | Reference-device20-minute route,200units/100buildings, frame and memory evidence |
| Two-client reality | Real UDP geometry fingerprint and reconnect/restart test; two native windows with independent cameras | Full current-release city/asset/action audit under reference network conditions |
| Art standard | One independent actual-client screenshot review and separate renderer fixtures | Five-biome/coast multi-view review after shader refinement; current UI observer wording |

No EXP/PLY/GEN/WORLD/UX checkbox is marked complete by this checkpoint.
The user's world-first sprint defers extra terrain scope while D0 is assembled;
this does not turn missing full-game evidence into a pass.
