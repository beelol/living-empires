# Always-visible generation progress

Base 3da99d25fc1bf53d23fe59ccae96a48f62d6a520. Primary UX-01, UX-04; affected
LIVE-10, GEN-08. Owned solo/public/app.mjs, index.html, style.css and this record.
Full updated README read, including the explicit prototype style-completion limit
exception. No provider, budget, package or native-world behavior changes.

A persistent map banner displays faction/request, Plan → Generate → Render →
Review → Install, real stage, attempt, elapsed time and queued target count. It
uses an indeterminate activity track for provider work and a separate exact
installed/total track for faction propagation. Existing requestStage,
effectiveRequestRevision and memberProgress helpers supply the state. Repairs
show their candidate number without resetting the true installed-building count.
Current failures retain the actual reason and last-good message. Details opens
the existing faction drawer; renders/history links use the current recorded job.

The banner has a reserved strip below the header; drawer, creator and help start
below it. Opening panels keeps it visible and never modifies typed drafts. No
browser interaction, runtime restart, provider call or push was performed.

Checks: node --check passed. Focused pure UI checks pass for rendering, queued
work, repairing candidate 2, persistent blocked reason, indeterminate activity,
and exact faction counts 1/5 → 4/5 → 5/5. New static DOM IDs exist; candidate-preview
is the existing dynamically created inspector node. Actual viewport verification
and reconciliation with the upstream economy UI are lead-owned and not_run here.

Maintenance follow-up: public foundry.maintenance explicitly labels queued work
as paused, removes provider animation from waiting requests, and retains the
recorded stopped reason. If an already-started request remains active, its actual
stage is shown with “Scheduling paused”; no claim is made that it was cancelled.
Focused checks include maintenance with queued and running work.

Replacement follow-up: a stopped banner entry must remain the latest relevant
request for at least one current member. Historical targetless failures and fully
replaced cancelled jobs remain in history without masking replacement progress
or completion. An unresolved failure on another member remains visible. Focused
checks cover same-revision cancellation → replacement running → installed, plus
a separate member failure and an obsolete targetless failure.
