# Premade construction must not dispatch art

Owner: parent integrator. Primary GEN-08, UX-04; affected SET-04/05, LIVE-10,
WORLD-05. Solo prototype fix explicitly requested by the user; not Bevy milestone
acceptance. Full README v2 read. Base main 50ffbd815f5eb949f6070d3d80b701627ce0e096.

Observed the fresh colony fdc28ee3-f3a9-4cce-a89e-2019bf69caa4 with automatic
Foundry disabled. Clicking premade solar/extractor still created explicit paid
requests, ahead of the research facility. These were new requests, not resurrected
old-colony work. Cancelled solar job job-helios-stepped-solar-array-1792971908219
and the queued extractor through normal APIs. Preserved research request
creation-ed082ac9-8d10-4648-a7d5-c16ec4d127a2. Dispatch temporarily paused while
fixing. History and $1,000 ledger are retained.

Worker automatic-generation-off owns solo/model.mjs, solo/foundry.mjs and the
model/faction/controller/server regressions plus its work record in an isolated
checkout. Parent owns this record and the build confirmation in solo/public/app.mjs; worker paths transfer only on explicit
handoff. Stable contracts: schema2, exact package activation, durable request IDs,
frozen costs/function, existing faction schemas. No schema or provider changes.

User's screenshot clarified all four premade buttons (solar, extractor, habitat,
workshop), including prospecting, must never enqueue or be auto-discovered for
art on ordinary placement, regardless of the toggle. Explicit custom creation,
regeneration, and faction restyling remain valid. With automatic generation off,
custom approval must not fan out automatic faction updates. Parent will preserve
main's newer target-function themes, palette validation and progress UI.

Checks pending: failing reproduction, worker full Solo suite, parent combined
Solo/npm/core checks on exact reconciled revision, independent review, existing
save preservation, live explicit-request dispatch, normal main push. No new paid
rehearsal or new colony is required: resume the user's existing request only.
