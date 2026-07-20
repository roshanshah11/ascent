# Ascent 0.6 offline technical documentation

This documentation set ships inside the native application and matches app
version 0.6.0. Ascent is a deterministic, agent-operable flight-dynamics
workbench. It is not a student-design tool, a commercial product, or a source
of unsupported flight-certification claims.

The bundled set includes:

- `EVIDENCE.md`: evidence scope, source authority, and validation limits.
- `CREDIBILITY.md`: credibility factors and regime qualifications.
- `CROSS_VALIDATION.md`: named cross-tool baselines and discrepancies.
- `SIXDOF_DERIVATION.md`: rigid-body equations, frames, and solver assumptions.
- `JOURNAL_FORMAT.md`: the canonical human and external-agent command seam.
- `EXPORTS.md`: deterministic review-bundle and geometry export contracts.
- `VERIFY_RELEASE.md`: checksums, signatures, SBOM, provenance, and platform verification.

Mission-review bundles reopen without network access. Ascent verifies every
member SHA-256, the bundle schema, and exact journal replay before displaying
the saved engineering state. Missing, stale, extrapolated, waived, or
low-confidence evidence remains qualified in the reopened review.
