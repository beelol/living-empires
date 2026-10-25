# Studio Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Recover Studio status refresh from stalled HTTP requests and make wait/failure reporting honest.
**Architecture:** Extract a single-dispatch JSON request helper with bounded cancellation, and a pure saved-job status presenter. Keep the existing job and renderer protocols.
**Tech Stack:** JavaScript ES modules, Node test runner, existing browser/Three.js Studio.
**Spec:** docs/work/foundry-studio-reliability.md (approved diagnostic handoff plus explicit unclaimed-only restriction).

## Global Constraints

Only claimed files. No server/provider/controller/launcher edits. No paid calls or running-service replacement. Preserve uncertain outcomes, independent acceptance, frozen budgets and evidence hashes.

## Review Focus

- A hanging response body must release polling just like hanging headers.
- A timed-out POST must never be automatically repeated or reported as definitely failed remotely.
- A routine successful status poll must not erase a failed cancel/activate action.
- Terminal durations stop at recorded completion, and missing timestamps do not fabricate durations.
- Newer upstream UI/claim changes must be preserved during main integration.

### Task 1: Bound Studio request waits
Files: public/request.mjs, public/app.mjs, test/request.test.mjs.
Interface: requestJSON(url, body?, options?) returns parsed JSON or an Error with outcomeUnknown on uncertain writes; exactly one request is issued.
- [x] Reproduce stalled header/body and verify one failed POST is never replayed.
- [x] Implement read/mutation deadlines, include the response body, and retain server rejection messages.
- [x] Route existing Studio API calls through the helper. Run focused request tests.

### Task 2: Preserve failures and show measured progress
Files: public/job-status.mjs, public/app.mjs, test/job-status.test.mjs.
Interface: describeJob(job, now) derives phase, elapsed text and terminal reason from saved records; never mutates the job.
- [x] Test active/terminal/missing timestamp, limits and error-display cases.
- [x] Add elapsed/current-phase information to the existing summary and separate polling errors from action errors.
- [x] Run focused and full suites, isolated fixture renderer smoke and browser inspection.

### Integration
- [x] Independently review combined owned changes including the native diagnostics worker.
- [x] Under the integration lock, fetch exact main, reconcile, check owned diff/tests, commit explicit paths and push fast-forward. Report exact revision and remaining live evidence gap.
