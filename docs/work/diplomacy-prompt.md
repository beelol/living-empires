# Bounded diplomacy prompt history

Owner: bilal-diplomacy-prompt. Base 05cf47d. Primary FAC-09;
affected FAC-02, FAC-06, DIP-03, AUD-04, WORLD-05. Owned only
dev/runtime/src/diplomacy.rs and this record. No full-game AC pass claimed.

Independent review found that 16 valid 500-character Unicode messages exceed
the 32768-byte request-context bound. The next record_attempt fails, permanently
stopping the worker even after restart. Escaped control characters are larger
still. Prompt history must be bounded by serialized bytes as well as event count.

Keep newest complete permitted history events, removing oldest prompt events
until the context fits. Keep identities, priorities, current offers and pacts.
Full authoritative history remains on disk; saved schema and action API stay v1.
GM owns transport/loop after 05cf47d and consumes exact saved source events;
this prompt-only change does not truncate its source history or add recipients.

Checks: reproduce with accepted maximum-length multibyte/escaped messages; verify
next attempt succeeds before and after restart, full history and pending offers
survive, and an unrelated faction receives no private messages. Run focused and
workspace Rust checks before the next normal main checkpoint.

Evidence: the new regression failed at the serialized-size assertion before
the fix (/private/tmp/argon-prompt-red.log). After the fix both emoji and escaped
control-character cases passed; 74 workspace tests and clippy passed on 05cf47d.
Independent source review found no further issues. Reconciliation with terrain
5a07994 adds only the terrain export/module/tests; prompt APIs and source history
are unchanged. The live host on that revision was healthy with Argon and 102
saved attempts, demonstrating continued ledger recovery across deployments.
After reconciling 5a07994, all 16 core/loop tests and workspace clippy passed.
The changed source passes rustfmt; workspace formatting reported only the new
terrain export's ordering in lib.rs, which was reported to that file's owner
and left untouched. Its preexisting difference is not part of this checkpoint.
