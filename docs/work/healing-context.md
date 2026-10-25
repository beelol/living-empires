# Repair context continuity

Owner: Sol / argon-healing-context. Base: b2f76c4.
Primary: LIVE-05, LIVE-06, GEN-08. Affected: GEN-02, UX-03, UX-04.
Owned paths: server.mjs and this record.

Dependency: art-quality owns the Rust provider request boundary. Agreed optional
request-only fields: GenerationRequest.componentBaseline (original Recipe),
ReviewRequest.componentEdit (existing frozen ComponentEdit), and
ReviewRequest.retainedBaseline (PreviousReviewEvidence). These are transport
context, not additions to any hash-covered contract, package or save version.
The world/village owner and its activation contract are untouched.

The creator receives both the immediately preceding candidate and the original
component baseline. An added component may be repaired on a later attempt without
mistaking its new ID for a collision in the original scene. Existing Rust component
guards continue to compare against the frozen baseline.

Later refinement reviews retain the three original captured baseline images in
addition to the preceding/current views. The comparative response remains bound
to the preceding candidate as required by the unchanged pipeline contract. The
critic must also preserve the original scene's strengths. Reviewer input hash
references are retained in job history; actual evidence bytes are validated before
dispatch. This supports independent review, not a deterministic guarantee of art
quality.

Scope: no automatic city/neighborhood propagation. The selected job's contract
stays frozen. Latest feedback can supersede previous creative choices within that
contract; changing the palette/theme contract requires a new bounded job. World
propagation still needs explicit prompt scope and authoritative activation.

Checks: node --check server.mjs passed; Rust with live-provider passed 65/65,
including original-baseline Add repair and scoped/retained-image critic inputs;
combined npm suite passed 59/59. Changes pushed as 0679df0 and served at 4318
with binary SHA-256 b338eb251f426195f9f56e25b2a5cfda7bc968b31e083d5afd9128a4f37b6b34.
Existing Argonaut and stadium package exports still pass the matching verifier.

Live evidence, 2026-10-25:

- Solar addition job `job-argonaut-1792967805134`: accepted hash
  `79c0c1e4f47f60e2dbed8ef180ae135d68ef18fdb2364512d88e17d63032fa73`.
  Added 14 parts in one new solar-array component. All 19 original parts and both
  original component records remained exactly equal. One Argon creation and one
  independent Flash review; no injected defect or fixture.
- Actual renders showed slab-like collector faces. A new scoped feedback job
  `job-argonaut-1792967879433` refined only solar-array, accepted hash
  `61cb539ebe29b0c6dfefb70a57e50ea7da24cec65c17fb0ff7b8cb6814faf13c`.
  Flash identified segmented photovoltaic panels and mounting articulation as
  improvements with no regressions. All 19 non-solar parts stayed exactly equal.
  Delta: 16 added, 8 changed, 3 removed parts, all within solar-array.
- Both jobs preserve three matched-camera before/after PNGs, hashes, feedback,
  call records, scope and independent comparisons. Exported packages and image
  hashes were verified; ignored `.data/solar-detail-audit.json` retains the audit.
- Dashboard discovered the API-created revision automatically. Review model was
  gemini-3.8-flash; creator/repair model gemini-4-argon. Shared conservative total
  allowance after these four calls: $37.3053925 of the approved $500, not an invoice.

Evidence boundary: these live jobs passed on their first candidate. The new
multi-attempt retained-baseline path and Add retry bug fix have regression-test
coverage, not a newly observed live multi-attempt example. No claim of general
artistic quality, world integration, neighborhood propagation or Bevy acceptance.
