# Independent Studio reliability fixes

Owner: Sol, this diagnostic chat; baseline e94a2a5. User approved implementation into main, then explicitly restricted it to unclaimed files with no coordination/messages to the other chat. Primary UX-04/LIVE-10; affected LIVE-04/LIVE-06/LIVE-07/UX-05. No game AC pass is claimed.

Design approved by the preceding diagnosis/handoff and narrowed by the user's ownership instruction: bound Studio request waits so a stalled status request releases the poll slot; preserve action errors instead of clearing them on the next ordinary status update; show elapsed and current-phase time honestly from saved timestamps. Keep all paid requests single-dispatch, report uncertain write outcomes, and preserve exact server job/evidence/approval protocols. Read-only status refresh can recover automatically.

Owned paths: public/app.mjs, public/request.mjs, public/job-status.mjs, test/request.test.mjs, test/job-status.test.mjs, this record and the matching plan. Native renderer process diagnostics are an independent bounded worker with its own claim/worktree/commit. The other chat owns controller/server/launcher/provider/renderer-delivery and colony UI; none are edited here.

Dependencies: asset owner's existing HTTP JSON API and saved job contract v2; elapsed times derive from createdAt/events/deadline, not invented progress percentages. Public UI helpers do not change shared schema, collision, clock, or persistence contracts. Main's newest summary UI is preserved. Rust binaries and Node dependencies used for rehearsal are private/read-only; running services and paid jobs are untouched.

Checks planned: focused Node request/status tests (timeout before headers and during body, HTTP rejection, no automatic POST retry, active and terminal time display, exact stop reason), full npm suite, Rust core build/tests, isolated Studio fixture smoke and actual browser inspection. Live latency/paid success remains not_run. Integrate with a normal fast-forward push only after the locked fetch/reconcile/check sequence; preserve dirty canonical checkout.

Ruling: startup, controller and renderer-delivery changes stay out of scope because their exact paths are actively claimed, per the user's explicit restriction. Existing fixes in main remain available but no other agent's experiment is imported.

## Evidence and update reconciliation

Reconciled the original e94a2a5 baseline with be076d4. Read all intervening changes, including the original-scene fallback/source identity, selected retained baselines, provider HTTP fixture and component-validation improvements; README contract v2 did not change. Preserved the newer scene-selection behavior. No claimed experiment or policy limits from the separate Foundry chat were imported.

Regression reproduction: the initial request tests failed 5/6 and status tests failed 7/8 before the fixes. Independent review caught a non-JSON 4xx being marked uncertain; its new regression failed before repair and now passes. The complete Studio npm suite passes 50/50; core build and 58 offline Rust tests pass. Logs: /tmp/foundry-studio-npm-test.log and /tmp/foundry-studio-core-test.log.

Isolated, credential-free Studio at port 4419 with private data passed npm run test:e2e: fixture defect repair, six real headless-renderer PNGs, exact activation/idempotency, cancellation exclusion, second-theme activation and rollback, custom frozen theme/export, and disabled live inference. This is offline rehearsal, not independent live-provider or native Bevy acceptance. Log: /tmp/foundry-studio-smoke.log.

An actual Chrome browser loaded the saved fixture scene and exercised polling with controlled HTTP faults: one read exceeded the real 10-second timeout, the next poll recovered, a failed cancel message survived subsequent successful polls, phase/elapsed information appeared, and a blocked saved job showed its reason with cancellation hidden. No page errors. Evidence: /tmp/foundry-studio-accepted.png and /tmp/foundry-studio-blocked.png; harness /tmp/foundry-studio-browser-check.mjs. A fixture HTTP rejection was injected, not a paid job or real cancel.

Independent native-worker review also identified partial public-frame replacement on filesystem failure. The worker is fixing staged publication/rollback within its separately owned paths before integration. Neither simulated subprocess checks nor the fixture smoke establishes the full native-renderer/live-review loop. Paid provider latency remains not_run; the earlier live histories measured 60–165 seconds and this change does not shorten provider generation.

Latest locked fetch advanced main to 722b9257908253c7e0500ad6337fbab8cd50da3b with live scene history. Its dashboard now polls the saved-job list every three seconds, including terminal jobs. Reconciled the Studio change onto that flow: preserved history/version inspection and follow-new-run behavior, used the upstream history connection notice, and kept bounded requests plus retained action/job errors. No duplicate per-job poll or connection UI was retained. Combined browser and suite checks follow this reconciliation.

## Combined acceptance before publication

Verified code baseline: cd56d4fedea11e7af32db4ef465cb262803c7ae8, fetched under the local integration lock. Its follow-up history copy and active-run summary were inspected and preserved. Studio implementation: 1d28a59; native diagnostics/publication: 272a261 and fd90aa5 (cherry-picks of worker 8f46104/e471700). Worker explicitly handed its three paths to Sol and released its claim; parent claimed them before integration. Changed-file ownership check passed for all ten paths against the exact integration base. Shared return formats, hashes, budgets and authoritative behavior are unchanged.

On the combined code: npm test passed 59/59; colony tests passed 39/39; cargo test -p living-world-core --features live-provider --offline passed 63 tests, including real localhost HTTP transport fixtures without provider credentials. Logs: /tmp/foundry-studio-combined-npm-test.log, /tmp/foundry-studio-colony-test.log and /tmp/foundry-studio-core-live-test.log.

Restarted only the credential-free isolated rehearsal server on 4419 from the combined code. Full npm run test:e2e passed again, with no busy skip (/tmp/foundry-studio-combined-smoke.log). Repeated the Chrome regression against the new three-second dashboard-list polling flow: all six checks passed, six list reads, zero page errors (/tmp/foundry-studio-browser-check.log). Inspected the resulting screenshot with live history controls and visible blocked reason. The canonical live services and dirty primary checkout were preserved.

Gates: focused/unit/regression/browser/offline rehearsal PASS; live paid creation plus independent review and actual Bevy renderer run NOT_RUN. No README world AC is marked passed. Provider generation latency, the separately claimed launcher/controller/delivery changes, and crash-atomic publication remain outside this stream. Publication uses a normal fast-forward origin/main push; the exact synced revision is reported in the task's final response after success.

Final pre-push fetch advanced to 2d05c79b9c05bd0d01dd131902a75e72a1d4704d, containing only two new dashboard-evidence documentation lines. Inspected and merged that exact revision; no executable code changed after the passing combined checks. Final ownership and whitespace checks passed against 2d05c79.
