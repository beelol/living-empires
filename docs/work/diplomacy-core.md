# Diplomacy core stream

Owner: diplomacy-core subagent, isolated worktree based on db64e4d.
Primary: FAC-02, FAC-09, DIP-05, DIP-14.
Affected: FAC-01, FAC-06, FAC-07, DIP-03, DIP-04, DIP-13, WORLD-02, WORLD-05, AUD-04.
Owned paths: dev/runtime/src/diplomacy.rs and this record.
Dependencies: parent owns module exports, runtime wiring and provider runner;
Gemini transport stream owns provider requests. Agreed proposal v1 has seven required
fields: action, target, message, summary, offer_id, offer_version, stance. All strings
except integer offer_version; unused strings empty, unused version 0, unused stance neutral.
The independent demo ledger makes no inventory, territory, combat, or native protocol changes.
Checks: real temporary-file tests for private contexts, exact consent, stale and expired
responses, request replay, service failures, restart, writer lock, corrupt and failed saves.

## Handoff

The parent stream `bilal-gemini-host` owns integration, exports/dependencies, the
provider runner, operator UI, live test, and final combined checks. This stream
hands its two paths to that owner after the scoped commit and releases its claim.

Implemented a separate saved ledger with three stable mutually known fixture
factions. Per-faction context includes only its own priorities, known public
attitudes, delivered history, and its offers/pacts. The all-faction snapshot is
operator-only and contains private request contexts and raw proposal evidence.
Offers are fixed version-1 non-aggression, expire after 120 seconds, and require
recipient acceptance of the exact offer before a 300-second pact is recorded.
These are diplomacy ledger effects only; there is no combat enforcement, economy,
territory, or game protocol change.

Every accepted/rejected bounded proposal gets a durable receipt; the same request
ID must retain both observed revision and proposal. Atomic saves happen before
acknowledgement, with an exclusive writer lock and corrupt-save refusal. Saved
attempt count is written before provider calls and survives restart. Attempts and
receipts stop at 1,024 records rather than evicting deduplication evidence. Context
contains the latest 16 delivered history entries. Calls that fail cannot later
apply a response. Model errors and stale proposals do not advance gameplay revision.

`record_attempt` requires the exact `context(actor, now)` using the same `now` and
a fresh request ID. `record_failure` is only for provider failures: `apply` already
records rejected proposals. Optional message text on stance/accept/decline is
delivered to the named other party; summary is private. Wait text is not delivered.
Time is supplied by the runner in seconds; a Unix clock means deadlines continue
while the process is stopped. This is not paused full-game simulation time.

Validation: eight focused file-backed core tests passed in an external Cargo
harness using the existing runtime dependencies; clippy with warnings denied
passed; rustfmt and repository formatting passed. The initial six tests failed
against the empty implementation before the core was written. A clock-rollback
regression also failed before its fix. Read-only review found a provider budget
edge when manual actions filled decision capacity; its regression failed before
the capacity guard was added. All 40 existing Rust workspace tests passed.
The existing Darwin linker unwind-size warning was retained. The broad Python
suite encountered `test_invalid_content_keeps_previous_host_and_version` timing
out its initial three-second fake content validation while other checks ran;
35 Python tests passed and this one failed; final rerun belongs to the integration owner.
Ownership checks passed for the two assigned paths. No external API call or game
acceptance audit was performed; no README criterion is marked passed.
