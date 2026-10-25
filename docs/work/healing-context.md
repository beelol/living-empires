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

Checks: node --check server.mjs; Rust request-boundary regression checks supplied
by art-quality; combined existing npm and Rust suites plus live evidence to be
recorded after integration. Until then live acceptance is not_run.
