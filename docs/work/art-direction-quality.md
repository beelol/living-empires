# Art direction quality stream

- Owner: `prompt_ui`; integration and live validation owner: lead (`/root`).
- Base: `b2f76c45854d0748dda71ebbc04144357020c2a8` (`origin/main` supplied by lead).
- Primary ACs: GEN-01, GEN-02, UX-03. Affected: LIVE-03, LIVE-04.
- Owned paths: `lib/theme-inference.mjs`, this work record. No test edits.

The inferred direction now establishes composition before ornament and describes purposeful geometry at primary, secondary, and smaller audit-visible scales. It requests a distinctive feature tied to the user's purpose, coherent supporting forms, material assembly expressed through layers/recesses/structural rhythm, and quiet areas that keep the scene readable. It avoids fixed part counts, disconnected decoration, and generic preset buildings. Unsupported styling is translated into available geometry and palette relationships, with limitations retained honestly.

Exactly three requested visual checks cover recognizable purpose, composition/material assembly, and distinguishing details visible in the existing audit views. These are instructions for new inferred contracts; no frozen job, acceptance schema, runtime limit, model setting, or prior evidence changes.

## Dependencies and interfaces

- Lead owns the `server.mjs` inference/freeze entry point and integration. The existing `inferTheme({prompt, apiKey, model, signal, create})` return shape and validated `themeSchema` at the recorded base stay unchanged; contract v2 freezes its result before creation.
- `provider_request` owns the concurrent `art-quality` creator/reviewer prompt stream. We agreed to consume existing `shapeLanguage`, `materialFinish`, `motifs`, and `visualChecks`, without new fields or quotas.
- The current `public/scene.mjs` renderer supplies box/cylinder/cone geometry, palette materials with architecture-family roughness/metalness defaults, and existing audit views. Lead/render audit owns actual captures and visual evaluation. No renderer change is required by this prompt.
- Rust remains the authoritative validator/controller. An inferred visual check does not grant gameplay capability or approve a candidate.

## Checks and evidence boundary

- PASS: `node --check lib/theme-inference.mjs`.
- PASS: `node --test test/theme-inference.test.mjs test/themes.test.mjs` — 10 existing tests, zero failures/skips. These check call/validation/error retention and fixture behavior, not artistic quality.
- No new test mirrors the added prose. No paid calls, integration, push, server restart, or release performed by this stream.
- NOT_RUN: live inferred theme, subsequent creative generation, independent actual-render review, and matching-camera improvement evidence. GEN-01/GEN-02/UX-03 remain unproven by this implementation; lead owns those live checks.

Handoff: commit this narrow prompt/work-record change for lead integration; retain the existing live budget, review independence, exact hashes, and frozen limits.
