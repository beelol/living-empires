# Native visual polish implementation plan

Goal: beautiful efficient presentation of native terrain and authoritative player state.
Spec and ownership: docs/work/visual-polish.md; canonical README.
Architecture: presentation module installed by visual.rs after handoff. Geometry and network state remain unchanged. Rust/Bevy 0.20.

- [ ] Build compact mission-control HUD, alerts, camera/control legend and bounded measured frame telemetry. Handle reconnects, long names, 1280x720 and 1920x1080.
- [ ] After terrain handoff, integrate lighting, atmosphere and efficient analytic water; preserve geometry, ground contact and camera controls. Offer optional effect toggle.
- [ ] Compile, focused tests, actual native renders and independent visual review; one bounded repair pass. Record revision and measured limits, integrate explicit paths with lock.
- [ ] Set recurring monitor for changed revisions; one bounded useful fix per run, quiet when unchanged. Stop this initial run by 20:19 UTC.

Review focus: no fabricated values, no private observations, no unbounded per-frame allocation, no UI overlap, no shader failure or collision displacement. Full reference-device performance remains blocked without required load and hardware.
