# Diplomacy demo limit recovery

Owner: bilal-diplomacy-limits. Base: 347c860. Primary FAC-09, UX-02;
affected AUD-04, WORLD-05, UX-04. No full-game acceptance pass claimed.

The active goal is the existing small Argon diplomacy demo, with every verified
working checkpoint pushed to main. This increment aligns the configurable call
budget with the existing durable ledger capacity and verifies live configuration
recovery. No new faction, action, network or persistence schema is introduced.

Owned paths are recorded in dev/agent.py. Expected edited paths: gemini.rs,
diplomacy.rs, diplomacy_loop.rs tests, .env.example, dev/README.md, this record.
Dependencies: the previous diplomacy-core/transport are integrated at 347c860;
terrain owns lib.rs/server.rs; Argon GM owns its new game_master module and
requests the existing transport/loop next. Leave its interface unchanged and
release these paths promptly after the bounded fix. No GM adapter is implemented
here. Messaging coordination was subsequently authorized explicitly in this chat.

Evidence before fix: Gemini configuration accepts DIPLOMACY_MAX_CALLS up to
10000, but both attempts and decisions stop at 1024. A later provider attempt
therefore stops the worker with a terminal ledger error. The chosen small fix
rejects budgets above the actual ledger capacity up front. Correcting the file
must recover automatically; reaching a smaller budget and raising it must resume
the next faction without replaying already saved decisions.

Checks: reproduce oversized-budget acceptance with a failing configuration test;
exercise recovery/resume with the real file-backed worker and only the external
provider replaced; run focused and workspace Rust checks; inspect live host
activation and preserve failed outcomes. Defaults stay 120 calls and Argon.

Verification: both new regressions failed against 347c860 for the expected
oversized-budget behavior. The first wider unit run could not bind local HTTP
fixtures under restricted execution (22 passed, eight permission errors); retained
at /private/tmp/argon-limit-unit-green.log despite that log's earlier chosen name.
The permitted full workspace run passed 71 tests. Reconciled to origin/main
a8984a6 (distinct Lua module fix and independent Studio bridge); the combined
workspace passed 73 tests, formatting, and clippy with warnings denied.
Logs: /private/tmp/argon-limit-combined.log and /private/tmp/argon-limit-clippy.log.
No default budget or real key changed. User explicitly authorized Argon-chat
coordination; GM agreed to take gemini.rs/diplomacy_loop.rs after this checkpoint.
Its future shared budget counts core attempts plus durable GM attempts, preserving
separate request IDs and core-only faction rotation. No GM behavior is added here.
