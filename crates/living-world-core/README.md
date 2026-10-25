# `living-world-core`

This first-party crate is the authoritative evaluator for generated world recipes. A model may propose a recipe, but the crate decides whether the active theme contract, material tokens, dimensions, ground contact, commander access, and sensor range satisfy the deterministic rules. Visual or other external evidence remains a separate acceptance input.

The library exposes recipe validation, collision-aware movement, deterministic checks, and a serialized `AssetPipeline` state machine. Create a pipeline with `AssetPipeline::create(theme, prompt, now_ms)`, set non-fixture `Provenance` before any work, charge provider calls, submit candidates, and decide them with independent theme/render statuses and image hashes. The frozen contract, candidate, and approval package carry SHA-256 integrity checks. Defaults enforce three candidate attempts, a twenty-minute active window, six provider calls, and exactly three distinct image hashes. Approved packages stay exportable after the active job expires.

The pipeline exports the `living-empires.asset` schema version 1 package used by the existing app. It includes the complete theme and recipe, manifest checks, evidence hashes, content and theme hashes, and provider provenance. A pipeline can be stored and restored through Serde; its hashes and limits are revalidated before each operation.

The `world-check` binary accepts one JSON request on standard input. For example:

```json
{"recipe":{"themeId":"argonaut","name":"Relay","description":"Outpost","sensorRadius":12,"parts":[{"id":"a","shape":"box","role":"support","position":[-1,1,0],"size":[0.5,2,0.5],"material":"frame"},{"id":"b","shape":"box","role":"support","position":[1,1,0],"size":[0.5,2,0.5],"material":"frame"},{"id":"c","shape":"box","role":"support","position":[-1,1,1],"size":[0.5,2,0.5],"material":"frame"},{"id":"d","shape":"box","role":"support","position":[1,1,1],"size":[0.5,2,0.5],"material":"frame"}]},"themeId":"argonaut"}
```

The legacy request omits `theme` and accepts only the three frozen preset IDs. For an editable theme, include the complete typed contract under `theme`; its ID must match both `themeId` and `recipe.themeId`. The contract validates all descriptive fields, bounded lists, palette colors, and enum values. Its exclusions are preserved as supplied, and `referencePreset` is only a rendering fallback.

The binary writes a JSON object containing `checks`, the `movement` result and trace, the engine version, and the validated `theme` when supplied. Invalid input or a rejected recipe produces a JSON `error` and a nonzero exit status. Provider HTTP support is optional and disabled by default; enable it with the crate's `live-provider` feature. Build and test from the workspace root with `cargo build -p living-world-core` and `cargo test -p living-world-core`. The crate is intentionally `publish = false`.
