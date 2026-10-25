# Studio bridge implementation plan

Goal: durable replay/evidence exchange ready for Studio and GM consumers.
Architecture: stdlib Python SQLite transaction journal and immutable blob store;
versioned JSON exports. No changes to existing world saves or game protocol.
Spec: ../specs/2026-10-25-studio-bridge.md.

Global constraints: exact owned paths only; shared-main index under integration
lock; no secrets in faction exports; no Studio activation or build authority.

Review focus: restart after uncertain commit; conflicting duplicate payload;
malformed/tampered event order; private images; stale replan acknowledgment.

## Task 1: history and evidence

- [x] Write failing tests in dev/studio/test_bridge.py for Journal append/reopen,
  corruption, dedup, concurrent writers, evidence hashes and deterministic replay.
- [x] Implement dev/studio/bridge.py using SQLite FULL synchronous transactions,
  canonical JSON/SHA256 records and blob insertion in the same database.
- [x] Run python3 -m unittest discover -s dev/studio -v, 18/18 green.

## Task 2: scoped delivery

- [x] Test GM envelopes, private-event exclusion, activation notices, persisted
  acknowledgment with stale-head rejection, export CLI behavior.
- [x] Implement Journal.export_gm(recipient), pending(recipient), acknowledge
  and an operator CLI with verify/export/replay commands. Document v1 transport.
- [x] Run targeted and available project checks; preserve failures in work record.

## Task 3: integration and follow-up

- [x] Record actual checks and unresolved live dependencies in work record.
- [ ] Coordinate consuming hooks with named owners; review exact owned diff.
- [ ] Commit and push verified checkpoint to main under whole integration lock.
- [ ] Create thread heartbeat for Pulkit Studio integration; notify meaningful
  changes only. Keep goal active until full live loop has required evidence.

Ruling: user explicitly chose continued autonomous implementation (“up to you”,
“continue”), so native execution without further design approval questions.
