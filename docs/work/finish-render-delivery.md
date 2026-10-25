# Renderer result delivery

**Owner:** `renderer_ui_diagnosis` / `foundry-render-delivery-01`
**Integration base:** `a47f993bbd1d626f520ce4baad7a05cf2600da5b`
**Primary ACs:** LIVE-04, LIVE-10
**Affected ACs:** LIVE-07, LIVE-13, UX-04
**Owned paths:** `public/render-delivery.mjs`, `public/render-worker.mjs`, `test/render-delivery.test.mjs`, this record.

## Contract and dependency

The server owns `/api/render-task` and validates the existing tuple of token, job ID, attempt number and candidate hash before consuming a result. A success response acknowledges the submitted images and movement; stale and malformed submissions return terminal client errors. The worker keeps the exact already-captured payload under its task token. A transport retry reuses that object and its frozen identity; it does not capture again, regenerate, review, or make a provider call.

Transient network errors, request timeouts, HTTP 408/425/429 and server errors can retry. Delivery is capped at five POST attempts by default, with each POST bounded to ten seconds. A terminal 4xx is treated as resolved locally to avoid looping stale or invalid evidence. If all attempts fail, the exact payload stays pending in the current page session but is marked exhausted; the worker does not recapture it or send beyond the cap. Closing or reloading the page discards that in-memory pending payload. The server retains its existing task timeout and cancellation behavior. No public schema, token, candidate/evidence hash, cancellation rule or server behavior changes here.

## Coordination

The parent completion/integration stream owns the server, provider and native-capture changes. The paired-adapter stream owns the launcher and Solo UI. This stream changes only the Studio worker delivery path and its helper. Cross-stream shared ACs are LIVE-10 and UX-04; the shared server payload contract remains unchanged.

## Evidence

- RED: `node --test test/render-delivery.test.mjs` failed four assertions because `createRenderDelivery` was missing.
- GREEN: focused delivery tests cover acknowledgement, retrying the identical payload after a transient failure, bounded final failure, and terminal stale/invalid responses.
- Full `npm test`: 63/63 pass after building the isolated `target/debug/world-check` binary. The first run before that local build had four missing-binary failures and one hit the parent-owned native-capture readiness race; no changes were made to that test path.
- Pending: ownership check, staged-diff review and commit SHA.

These tests establish only the renderer transport behavior. They do not establish live provider success, independent review, package activation, or integrated game acceptance.
