# Native client build and launch

Owner: foundation thread 01a171d8-149a-7613-bd7d-1eb488e82b22.
Owned paths: dev/native-clients.sh, dev/test_native_clients.sh, this record.
Terrain owner explicitly agreed this separate helper lane; primary checkout's
registry slot belongs to terrain, so this disjoint record records ownership without
overwriting its claim. Commit/index operations remain integration-lock serialized.
Primary WORLD-01, WORLD-03; affected LIVE-09, AUD-03, PLY-04. Full README read.

## Contract

Use the running host's health release SHA, archive that committed tree, build one
native binary, then launch separate session slots against that host. Cache exact
binary hash and commit in a receipt; reject changed cached bytes and a host release
change during build. Never read dirty source into the build or replace an active
binary. A build lock prevents duplicate builds. Keep existing world and server
process untouched. No Python dependency. Requires Git, Rust, curl, jq, tar, shasum.

Terrain owns all protocol/client/world integration. Its join HOST:PORT --session
PATH interface remains stable across planned v2. This helper cannot make a v1
server into terrain, supply approval, or claim observer-only behavior before that
client revision lands. Both processes have independent cameras and identities.
Server health is development LAN metadata, not a signed public distribution feed.
Only locally available commits are built; missing objects need the ordinary
coordinated fetch, not silent changes to the checkout.

## Plan and usage

1. Shell tests exercise actual Git snapshots with fake build/network tools: exact
   source, cache integrity, unhealthy host refusal, changed release refusal,
   separate session paths, failed build retention and concurrent build exclusion.
2. Implement helper and run shell syntax/tests, then real host probe and native
   launch. Record exact revision and distinguish local probes from second devices.
3. Publish explicit paths under integration lock, verify remote SHA and host health.

Commands from repository root:

```
sh dev/native-clients.sh build 127.0.0.1:4000
sh dev/native-clients.sh probe 127.0.0.1:4000 2
sh dev/native-clients.sh launch 127.0.0.1:4000 2
```

On another device use the host's LAN address instead. The final count is 1 or 2.
Close each window normally; Ctrl-C on the launcher stops only its own children.
Repeated calls reuse slots scoped to that server and preserve their saved identity.
Lua updates need no rebuild. Native/protocol changes require rerunning the helper;
this initial helper does not forcibly replace someone else's running client.

## Verified evidence (2026-10-25)

Shell syntax and fixture checks pass, including separate probe and real-player
slots, cached-byte tampering, server revision changes during build, build failure,
unhealthy host, invalid address, lock exclusion and dirty-source exclusion.
Fixtures use fake Cargo/curl and cannot prove a real connection.

Actual helper build of serving faa17d7 took 65 seconds with native dependency
recompilation; two actual Lightyear probe clients connected. Next serving cebe62f
built in 3.89 seconds with the warm cache. Two native macOS bundle processes then
joined as persistent commanders 8 and 7. Actual captured windows show Connected,
the same eight commanders/world/Lua hash, and distinct local selections. Panning
client 1 changed only its camera; client 2 retained its view. The host advanced
through 17e478e to e1b207b during inspection; both clients reconnected, retained
their identities/positions, and displayed the matching current host revision.
The binary remained cebe62f; the compatible protocol remained version 1.

Local captures: .local/native-client-evidence/client-1.png, client-2.png,
client-1-panned.png, client-2-after-pan.png. Raw probe receipts contain session
capabilities and remain private under .local/native-clients/sessions/.
The initial direct executable launch lacked macOS application identities; bundle
wrappers fixed inspection. An empty assets directory avoids missing-watcher warnings
for the primitive development scene. No generated art or terrain is claimed here.

These are two processes on one Mac, not two physical devices, reference-load
performance, full WORLD-01 acceptance, or approved LIVE content activation.
Cold native compilation is not seconds; measured warm native compilation is.
