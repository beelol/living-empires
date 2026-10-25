# Persistent faction areas and usable update flow

Owner faction-area-integration (Sol). Base c2a445c. Primary subjects GEN-08, WORLD-05, UX-01, UX-04; affected GEN-02, LIVE-10. Owned server, server tests, this record. The user explicitly requests refinement of First Landing appearance groups; README v2 native camera-only/ownership contract is unchanged.

Dependencies: faction-area-state owns compatible optional area bounds/order on solo save v2, authoritative new-building membership and scoped preferExistingDesign requests. faction-area-adapter owns exact accepted design reuse with unchanged Rust/image verification. faction-area-ui owns persistent region preview and explicit Update faction / Save faction area controls. Foundry API, package and hash-covered contracts remain unchanged.

Server cancellation must stop an inheriting request locally without cancelling the shared accepted source job. Commands preserve in-flight request object identity and new faction-area fields. Acceptance: scoped area API/new construction/restart, reuse/stale/rollback/failure checks, real browser update button/area selection/new construction and exact version propagation. Live checks pending; no native world milestone is claimed.

Combined verification: `node --test solo/test/*.test.mjs` passes 73/73 (zero skips), including saved-area API restart and cancellation isolation. Live UI and new-building verification pending.
