# Replay and Studio bridge

The user delegates format/design choices and asks to continue implementation.
Use Python standard-library tooling alongside existing dev scripts, avoiding
changes to other streams' Rust schemas. This is an adapter, not a second world
authority or decision scheduler. The user's latest direct-main instruction
supersedes worktree guidance for this stream.

Approaches considered: YAML diffs are readable but require another parser;
seed-only replay cannot reproduce model decisions or changed asset bytes.
Choose canonical JSON hash-linked commits plus content-addressed binary evidence.
Each commit contains parent hash, event timestamp in UNIX UTC seconds, simulation
tick, release/rules/assets/seed identity, accepted input and resulting state.
State diffs reconstruct recorded states; an optional pinned reducer verifies
actual deterministic rules replay against every recorded result. Never call a
recorded model answer live generation. Existing worlds require an explicit
starting snapshot; no invented earlier history.

Store one append-only journal and immutable evidence blobs. Lock writers,
fsync before acknowledgment, reject corrupt/truncated history, make duplicate
source IDs idempotent only when the complete input matches. Evidence records
capture event identity, render/client/content versions, viewpoint and timestamps;
missing actual screenshots remain missing. No fabricated screenshots.

Trusted operator APIs can inspect all records. Faction exports include only
explicit allowed recipients and no hidden parent/state hashes or global cursor.
GM v1 envelope uses at:u64 UNIX UTC seconds. Content-active notices reference
the authoritative activation and exact content fingerprint. They wake replanning
but grant no units, territory or free builds. Durable acknowledgments reference
the current notice set and decision-state hash; delivery is at-least-once.

Live hooks and network transport require explicit owner handoffs. Pulkit's
transport is not available yet. Mark that integration blocked until real
authority -> bridge -> Studio -> approved activation -> faction paid action
has actual client evidence. F0/D0 first; full-game criteria stay unchecked.
