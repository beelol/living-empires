# Gemini transport stream

- Owner: `gemini-transport`; base: `db64e4d40dd2f0e8ec0fde5801c2a40d23272bea`.
- Primary ACs: FAC-02, FAC-09, DIP-14. Affected: AUD-04, DIP-03, DIP-05, UX-04, UX-06.
- Owned paths: `dev/runtime/src/gemini.rs`, this record.
- Parent owns dependencies, library export, host loop, configuration example, and integration.
- `diplomacy-core` owns filtered observations and authoritative proposal validation.
- Shared v1 interface: `load_config(&Path) -> Result<GeminiConfig>` and
  `decide(&GeminiConfig, Value) -> Result<Value>`. Config reads the explicit file
  each time; environment overrides file settings without global mutation.
- Decision fields: action, target, message, summary, offer_id, offer_version,
  stance. All strings except nonnegative integer offer_version. Empty unused IDs,
  version 0, neutral unused stance. Actions are proposals only. Core validates
  recipient/version/consent; messages do not grant world effects. A proposed
  nonaggression pact lasts 300 seconds after recipient acceptance; its offer
  expires after 120 seconds. Each request uses only the supplied faction context.
- Checks planned: isolated config parsing and late key loading; real local HTTP
  transport success, rejected/blocked/incomplete/malformed/oversized responses,
  redirect refusal, timeouts, and secret-safe errors; baseline runtime tests.
- No live paid requests from this stream. Parent performs the authorized bounded
  live check with the user's local key after integration. Implementation tests
  do not establish a game AC pass or an independently approved release.

## API evidence

Checked Google's [Flash model documentation](https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash)
and [structured-output REST documentation](https://ai.google.dev/gemini-api/docs/generate-content/structured-output?hl=en)
on 2026-10-25. The current documented REST field is
`generationConfig.responseFormat.text` with `mimeType: APPLICATION_JSON` and
`schema`. User requested Flash while Argon becomes available; model remains
configurable through `GEMINI_MODEL`. Requests are bounded to 20 seconds and 2048
output tokens, with no retry or synthetic decision. A capped/incomplete reply
must be recorded as a failure, not a completed action.

## Validation

- Red phase: the missing-file config test and real HTTP success test failed
  against explicit unimplemented stubs before implementation.
- Scoped module harness: 14 tests passed, including chunked size limits, broken
  HTTP bodies, outer and inner invalid JSON, safety blocks, truncated model
  completion, thought-part exclusion, strict schema, redirect refusal, status
  redaction, timeout, dotenv parsing, precedence, and absent/late key loading.
- Harness `cargo clippy --all-targets -- -D warnings`: passed.
- `rustfmt --check --edition 2021 dev/runtime/src/gemini.rs`: passed.
- `dev/agent.py check --id gemini-transport --base db64e4d40dd2f0e8ec0fde5801c2a40d23272bea`:
  passed for the two owned paths.
- Unchanged runtime suite: all 30 tests passed with local sockets enabled and
  `--test-threads=1` (1 CLI, 13 content, 16 host). Initial run failed before
  compilation because the default target directory was outside the allowed
  write area. The next run with a temporary target passed content tests but
  seven HTTP tests failed to bind local sockets. An authorized socket-enabled
  run during parallel dependency compilation hit one-second timing guards in
  `native_collection_operations_cannot_bypass_instruction_limits`,
  `native_pattern_work_cannot_stall_loading_or_movement`, and
  `special_files_cannot_block_content_loading`; all passed on the serial run.
- No live provider request made by this stream. Parent must export the module,
  add reqwest 0.12 blocking/json/rustls-tls and dotenvy 0.15 dependencies, run the
  combined suite, and perform the authorized live demo. These remain handoff
  dependencies; no full-game AC is marked passed.
