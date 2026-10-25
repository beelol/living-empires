# Live city Lua activation

Foundation owns World/host activation and this record; city owner handed off the
single source and bounded reader at2777a9f. Primary LIVE-01/LIVE-02, affected
SET-02,WORLD-05. No generated-content release or full-game AC pass is claimed.

The sole source is dev/content/city_layout.lua, included in the content manifest.
The existing100ms content poll stages changed bytes into three long-lived faction
planners. FixedUpdate validates all six species/founding branches and promotes a
valid staged policy, including when no construction is requested. Rejected syntax,
looping callbacks, malformed files or invalid outputs retain the active policy.
Health distinguishes observed source hash, active per-faction hashes and errors.
Future paid proposals use the retained planner; existing buildings do not move.
No extra warehouse/need/model call is fabricated merely to demonstrate reload.

Real host test game/tests/city_reload.rs: PID32452 remained the same; a valid edit
activated in265.565ms and placed the next paid helios generator at[1460,1536]
instead of the previous policy's[1420,1536]. All nine paid buildings completed.
Syntax and infinite-loop edits each reached a distinct observed hash and rejection,
while active hashes and saved world data remained unchanged. Passed10.76seconds.
This is an actual socket/file-edit/authority/save proof, not a mocked file watcher.
The test uses a temporary world; canonical4000 promotion is tracked separately.
