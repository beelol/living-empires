# Parallel agent workflow

This repository is intended for many small streams, including roughly 20 streams
per person across 2–3 people. Each stream reads the full `README.md` and follows
`AGENTS.md`. File separation alone does not prevent incompatible rules: agree on
shared contracts before implementing dependent work.

## Start one stream

1. Read the full acceptance checklist and inspect the current implementation.
   Record a short brief: stream/person, primary ACs, affected ACs, exact owned paths,
   dependency owners and contracts, applicable cross-cutting rules, and checks.
   Keep each stream's record separate; do not make every agent edit one ledger.
2. Inspect active claims. Choose a small coherent change whose dependencies are
   available or have an agreed stable interface. Give high-conflict files one
   owner; request a handoff for shared schemas, dependency files, or common setup.
3. Create an isolated worktree from the intended current baseline. Use the app's
   worktree tools when available, or an ordinary Git worktree. Each has its own
   branch or detached HEAD, files, and index. Never bypass Git's protection against
   checking out `main` simultaneously in multiple worktrees.
4. From that worktree, claim the paths before editing:

   ```sh
   python3 dev/agent.py list
   python3 dev/agent.py claim --id bilal-economy-01 \
     --acs SET-05 --affects SET-06 WORLD-05 \
     --paths server/economy/ tests/economy/
   ```

   Replace paths with real planned ownership. A trailing slash covers a subtree;
   exact paths also cover descendants. Quote globs so the shell does not expand
   them. IDs are checked against the current README. Related ACs produce a warning
   to coordinate, not exclusive ownership of the whole criterion. Claiming an AC
   does not mean it is implemented or passed.
5. Implement and verify within that ownership. If the scope changes, coordinate
   first; release and reclaim with the revised paths before editing them. If another
   stream claims the gap, resolve the ownership conflict before proceeding.

## Check and hand off

```sh
python3 dev/agent.py check --id bilal-economy-01
python3 dev/agent.py check --id bilal-economy-01 --base <integration-base-sha>
```

The first command checks staged, unstaged, and non-ignored untracked files. The
second also checks the committed tree difference from the explicit base to HEAD.
Use the base on which this candidate was reconciled; otherwise upstream changes
may be mistaken for your work. Renames require ownership of both old and new paths.
Run the check in the claim's owning worktree, including after incorporating drift.
Keep ignored generated files out of commits unless they are explicitly part of
the deliverable; this helper does not inventory ignored files.

Then stage explicit paths and review the staged diff. A handoff includes the
candidate commit(s), base SHA, primary/affected ACs, dependency decisions, and
actual validation results. Check unfinished work as carefully as finished work:
stubs and mocked dependencies cannot pass a real gameplay requirement.

## Integrate directly into main

Use one designated integration checkout per clone; workers do not share it.
An integrator may be the stream's own agent while it exclusively holds the slot.
Temporary worker branches are staging areas; this workflow does not require PRs.
The integration checkout's `main` is updated only by its current lock holder.

The local lock wrapper runs an explicit command without doing any Git operations:

```sh
python3 dev/agent.py lock -- sh
```

Perform the whole sequence inside that locked shell, then exit it. It is also
possible to wrap an existing integration script. Do not acquire the lock for just
the push or launch background Git jobs that outlive the locked command.

1. Confirm the integration checkout is clean and exclusively yours for this slot.
   Fetch `origin/main`, record its SHA, and inspect upstream changes. Never discard
   unexpected files. If no remote or `origin/main` exists, finish local preparation
   and report that publication still needs repository setup authorization.
2. Reconcile the candidate with that latest revision in the isolated worker
   checkout. Review semantic overlaps even if Git merges cleanly. Coordinate
   changed contracts; rerun relevant tests and ownership checks against the new
   base. Do not rebase or rewrite commits that have already been published on main.
3. Fast-forward the integration `main` to the verified remote baseline, then
   cherry-pick the reviewed candidate commits (or fast-forward to a candidate that
   extends that baseline). Review the final diff; it must contain only the intended
   stream's changes. If resolution changes code, redo affected validation before
   publication. Never continue an unresolved or unexpectedly dirty integration.
4. Confirm the resulting history descends from the fetched remote SHA and run the
   required checks on the result being published. Keep commit messages linked to
   AC IDs and preserve the validation record.
5. Push a normal update with `git push origin HEAD:main`. The server's ref update
   rejects an incompatible concurrent advance. This is the publication race check;
   it does not detect semantic conflicts or reserve ownership across computers.
6. On rejection, fetch again and reconcile the unpublished candidate with the new
   remote tip. Inspect drift, recheck ownership, and rerun affected checks before
   another normal push. Never force-push, blindly retry, or replace others' work.
7. Verify the published revision, exit the locked shell, and release the claim
   from its original worker checkout:

   ```sh
   python3 dev/agent.py release --id bilal-economy-01
   ```

If another stream needs your paths before integration, make an explicit handoff
that names the new owner, commit, remaining checks, and dependency state.

## Coordinate people and separate clones

The helper stores claims under Git's common directory, in
`agent-coordination/<stream-id>.json`. Linked worktrees of **one clone** share those
claims and locks. Separate clones, computers, or users do not. Git push races alone
cannot stop two people implementing contradictory behavior before publication.

Before multiple clones work in parallel, agree on a shared ownership mechanism:

- Assign non-overlapping module/path lanes to people, with one owner for every
  shared contract. Within each person's clone, use the helper to divide that lane
  among streams. Publish a separate durable record for each stream and read other
  people's active records before starting or changing scope.
- A team may keep `work/claims/<person>-<stream>.json` records on an agreed shared
  coordination branch, or use individual GitHub issues in an agreed queue. Each
  record should contain owner, ACs, paths, dependency contracts, base SHA, status,
  and evidence/handoff link. Do not use a single tracked file edited by all streams.
- When using Git records, fetch and scan **all active records**, publish only the
  stream's record using a normal fast-forward push, and begin work only after that
  publication succeeds. A rejected claim publication requires fetching and scanning
  again before retry. Close the record on handoff/completion; do not silently erase
  another owner's reservation. The helper does not automate or read these records.

External publication and messages require the authorization applicable to the
session. These instructions do not authorize posting issues, contacting people,
creating remote branches, or sending messages on their own. Until a shared mechanism
is agreed and available, do not assume local claims protect cross-clone overlaps.
Work only inside already agreed ownership lanes and surface unresolved ownership.

## Helper guarantees and limits

- Standard library only, Python 3.9+, macOS/Linux. Claims are serialized with
  `flock` and written atomically. Simultaneous conflicting claims cannot both win
  within one clone. The integration lock fails immediately if already held.
- One claim per checkout; claim IDs cannot overwrite an existing claim. Only the
  owning checkout can check or release it. No automatic timeout can steal work.
- Path conflicts are conservative: literal ancestors conflict, and a glob reserves
  its entire literal directory prefix for conflict detection. For example,
  `server/*.py` conflicts with `server/config.json`, though the latter is **not**
  owned by that glob during `check`. Wildcards use Python `fnmatch` semantics
  (`*` can span slashes). Prefer exact paths or small subtrees.
- Claims, ownership checks, and locks are advisory, not Git hooks or access
  controls. They cannot prove an agent read all ACs, discover undeclared semantic
  dependencies, prevent manual edits, or validate gameplay. All participants must
  follow the agreement. Use code review and behavioral evidence as well.
- A crash does not expire a claim. Resume from the owning checkout. For an orphaned
  claim, first establish that its owner stopped and preserve any work; only then
  have the coordinator remove that specific JSON record while holding the registry
  lock. Never delete the registry or lock files to bypass an active process.

Run the helper's real-Git behavior tests with:

```sh
python3 -m unittest discover -s dev -p 'test_agent.py'
```

These tests cover local coordination, not the game's acceptance criteria.
