# `living-world-core`

This first-party crate is the authoritative evaluator for generated world recipes. A model may propose a recipe, but the crate decides whether its frozen theme, material tokens, dimensions, ground contact, commander access, and sensor range satisfy the deterministic contract. Visual or other external evidence remains a separate acceptance input.

The library exposes recipe validation, collision-aware movement, the standard access scenario, and deterministic checks. The `world-check` binary accepts one JSON request on standard input. For example:

```json
{"recipe":{"themeId":"argonaut","name":"Relay","description":"Outpost","sensorRadius":12,"parts":[{"id":"a","shape":"box","role":"support","position":[-1,1,0],"size":[0.5,2,0.5],"material":"frame"},{"id":"b","shape":"box","role":"support","position":[1,1,0],"size":[0.5,2,0.5],"material":"frame"},{"id":"c","shape":"box","role":"support","position":[-1,1,1],"size":[0.5,2,0.5],"material":"frame"},{"id":"d","shape":"box","role":"support","position":[1,1,1],"size":[0.5,2,0.5],"material":"frame"}]},"themeId":"argonaut"}
```

It writes a JSON object containing `checks`, the `movement` result and trace, and the engine version. Invalid input or a rejected recipe produces a JSON `error` and a nonzero exit status. Build and test from the workspace root with `cargo build -p living-world-core` and `cargo test -p living-world-core`. The crate is intentionally `publish = false`.
