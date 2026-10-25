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
