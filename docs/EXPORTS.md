# Geometry exports

`app/src/core/stl.ts` exports deterministic binary STL files from the same
pure model-tree mesh used by the viewport. Each geometry-bearing part becomes
its own closed solid, is parsed back, and is validated watertight before the
export tests accept it. The exporter has no runtime dependency and never
mutates the document.

## STEP is deliberately deferred

STEP is not emitted in this release. The current geometry source is a
procedural triangle mesh, not a B-rep model with analytic surfaces, topology,
and sewing operations. Writing a file labelled STEP from that representation
would be misleading and would require either a modeling kernel or a new,
justified B-rep implementation. STL is the honest interoperable output until
that source representation exists.

## Mission-review bundles

v0.6 review bundles are deterministic, versioned offline containers built by
`MissionReviewBundle`. Their manifest SHA-256-binds the saved document,
journal, predicted trace, typed events, selected alignment, reconciliation,
credibility report, validation-case manifests, report model, standalone HTML,
normalized PDF, dimensioned fin template, and longitudinal CG/CP layout.

Reopen verifies every member hash and proves the journal reproduces the saved
document before exposing the review state. Evidence qualifications are
mandatory and remain visible in the report model and offline HTML. PDF
determinism uses a fixed producer, no timestamps, and stable object order;
platform-signed installers are verified through signatures and provenance and
are deliberately not described as byte-identical.
