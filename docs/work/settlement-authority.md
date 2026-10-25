# Saved settlement authority

Foundation owns dev/runtime/src/settlement.rs, tests/settlement.rs and this record.
World/lib handoff is pending terrain's native checkpoint. Primary SET-01–06;
affected SET-09, WORLD-02–05, FAC-12, GEN-08. Full README previously read; incoming
workspace prefix reviewed. No game AC is passed by isolated module tests.

Persist construction ledger, reservations, starter identities, claims, buildings,
revision and event receipts inside World's existing atomic save. No second writer.
Stable diplomacy IDs: helios robots, tide organic aliens, cinder cyborg aliens.
Authored starter recipes have explicit provenance, fixed positive costs and bounded
geometry; they are not generated or independently approved Foundry releases.
City planner proposes; exact terrain placement validator and World accept/debit.
Render snapshot derives from this same saved geometry. Keep inventories private.

Tests cover idempotent charge, conflicting retry, stale placement, insufficient
funds, progress/recovery, and no mutation on refused placement. Actual World save,
network snapshot and rendered cities remain the next integration steps.

Checkpoint implementation: authored core/generator/storehouse cost 100/60/40,
120 work ticks, explicit 1000 starting metal per faction. Opaque placement proofs
resolve the terrain geometry; saved public BuildingView carries exact reservations,
progress and species. Retried faction/request IDs return the same building; changed
terms reject. Atomicity is clone-then-swap in the module and must be wrapped by
World's disk commit. Saved validation checks identities, recipe terms, balances,
geometry bounds and ledger/progress consistency. Five tests (including two shared
placement kernel tests) and clippy with warnings denied passed. World persistence,
authoritative work availability, renderer and network integration still pending.

World integration evidence: schema 3 adds settlements and explicit starter builder
anchors in the existing atomic save, preserving schema 1 arenas and schema 2
terrain worlds. Explicit initialization searches valid terrain; three paid cores,
generators and stores on seed72913 leave 800 metal per faction. World replay returns
the saved request receipt before replanning. Fixed simulation work saves progress;
a failed disk replacement leaves the live state unchanged. Exact nine-building
restart comparison exposed one-bit f64 parsing drift; enabling serde_json's
float_roundtrip restores exact saved geometry. No dependency version changed.
Movement rejects segments crossing reserved footprints; authored interiors remain
unavailable, so this is not a full entrance/traversal AC pass.

On immutable757ee07+city318e73e+authority source: city10 passed (isolated finalizer
probe runs as child), settlement5 passed, World3 passed, clippy library passed.
Host16 initially hit seven 2-second startup timeouts during parallel machine load;
unchanged serial rerun passed all16 including actual socket/restart/retry cases.
Earlier permission-restricted host failures were rerun with loopback permission.
Native city renderer/server hooks remain a separate immediate integration slice.
