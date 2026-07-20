# Ascent v0.6 — Mission Review Cinema

*Revised 2026-07-19 from primary-source research summarized in `docs/research/2026-07-19-v06-mission-review-research.md`.*

## Release thesis

v0.6 turns Ascent from a simulator with evidence features into a **time-native flight campaign review system**. It must show what was predicted, what actually happened, where and why they differed, what a proposed change would do, and the complete evidence trail behind the decision.

This is not a student, hobby, competition, or commercial-product release. The intended operator is a GNC or flight-dynamics engineer at a new-space or defense-tech startup. OpenRocket remains only a useful low-fidelity regression fixture. The bar is Basilisk-grade architecture, Nyx-style named-baseline validation, RocketPy-class atmospheric flight physics, GMAT-style technical credibility, and a workbench an external AI agent can operate through the same audited command seam as the human.

The cinematic surface is not decoration. Validation cases, imported telemetry, residual metrics, the synchronized viewport, proposal ghosts, journal replay, and exported briefings are projections of one temporal evidence model. If a state or conclusion is not supported by that model, the interface does not invent it.

The standing invariants remain law: one dispatcher for mutations; byte-identical journal replay; deterministic studies; immutable raw imports; no runtime network; every derived artifact linked to parent hashes; render layers dispatch zero commands; previews use cloned state; approval dispatches exactly one canonical batch.

## Architecture: one evidence spine, three clocks

The release introduces three foundational contracts:

- `FlightTrace`: versioned 6-DOF truth, estimate, sensor, environment, configuration, and event channels with explicit units, frames, time bases, validity masks, uncertainty, source hashes, and derived-channel lineage.
- `TelemetryBundle`: immutable multi-source raw and calibrated streams with clock domains, frame definitions, calibration records, sensor/estimator provenance, and adapters into `FlightTrace`.
- `EvidenceManifest`: intended use, validity domain, input pedigree, evidence level, frozen thresholds, transformations, caveats, and hashes linking raw data to every derived result.
- `ReviewClock`: one playhead mapping simulation time, explicitly aligned measured-flight time, and journal/campaign time without collapsing them into one raw timestamp.

Every v0.6 feature consumes these contracts. No report-only schema, viewport-only timeline, or reconciliation-only data model is allowed.

## Internal gate A — Evidence Spine

### Step 1 — Establish the temporal evidence kernel

Design and implement `FlightTrace`, `TelemetryBundle`, `EvidenceManifest`, typed review events, and `ReviewClock` as small versioned domain modules. Model position, velocity, attitude quaternion, body rates, acceleration/specific force, environment, configuration/actuator state, uncertainty, frames, and time bases without assuming every source supplies every channel. Store raw, calibrated, estimated, simulated, derived, and presentation tracks separately. Event records carry detector id/version, confidence, source sample ranges, and evidence hashes. Camera tracks are presentation-only and can never alter scientific state.

Acceptance:

- Canonical serialization is byte-identical and rejects non-finite values, unknown required versions, invalid units/frames/time bases, broken parent hashes, invalid quaternions, and non-monotonic source clocks.
- The review clock maps simulation, measured, and journal domains through explicit transforms and never rewrites original timestamps.
- Existing single-flight and dispersion results adapt into traces without changing their golden numerical summaries.

Out of scope: live telemetry transport, arbitrary user scripting, or a second persistence system.

### Step 2 — Productize the validation evidence ladder

Build a checked-in validation library whose case manifests encode NASA-style intended use, evidence level, input pedigree, source authority, uncertainty, validity domain, frozen acceptance thresholds, and caveats. Layer the evidence from analytic/unit truth through OpenRocket compatibility and RocketPy cross-validation to at least one license-cleared full-scale flight dataset. Treat OpenRocket as a low-fidelity regression reference, never the acceptance ceiling. Run every case through ordinary study and evidence seams.

Acceptance:

- Each case records provenance, license, immutable fixture hash, configuration mapping, metric definitions, uncertainty, thresholds, and known mismatches before evaluation.
- Changed fixture bytes or mappings invalidate prior results; identical reruns are byte-identical.
- The UI and report distinguish evidence levels and caveats instead of reducing credibility to one pass/fail badge.

Out of scope: runtime downloads, undisclosed fixture transformations, or accuracy claims outside a case's declared validity domain.

### Step 3 — Ingest professional telemetry as immutable evidence

Implement a schema-driven telemetry ingestion layer for arbitrary CSV/JSON/binary-to-normalized adapters, with reference adapters for PerfectFlite, RRC3, and Blue Raven only as edge compatibility tests. Ingest multi-rate IMU, GNSS, barometric, estimator, environment, configuration/actuator, and event channels into `TelemetryBundle`. Preserve raw bytes, source/device/firmware/export metadata, clock domain, coordinate frame, calibration, units, column or packet mapping, sensor uncertainty, validity mask, and importer provenance. Every transformation produces a derived artifact with parent hashes rather than overwriting source evidence.

Acceptance:

- Golden multi-source fixtures import with deterministic normalized traces, explicit SI conversion/frame mapping, packet or line errors, and untouched raw data.
- Ambiguous units or columns require an auditable user choice; no silent heuristic becomes canonical evidence.
- Import, undo, journal replay, project migration, and study staleness preserve the one-dispatcher and byte-identical replay invariants.

Out of scope: cloud sync, live radio ingest, a hardware-driver ecosystem, or automatic device detection without a recorded decision.

### Step 4 — Align clocks without hiding physics error

Create an alignment artifact supporting manual relationships, event-based correlation, and constrained offset/drift estimation across multiple sensor, estimator, simulation, and vehicle clock domains. Store time-system definitions, algorithm and version, parameters, overlap interval, objective, confidence, discontinuities, and untouched raw clocks. Resample only inside valid overlap. Unconstrained dynamic time warping is prohibited from official scores.

Acceptance:

- Synthetic shifted/drifted fixtures recover known alignment within frozen tolerances and report confidence and failure modes.
- Manual, detected, and optimized modes yield deterministic artifacts and can be compared before one is selected through a journaled command.
- Missing overlap, weak launch evidence, clock discontinuities, and excessive drift fail visibly without fabricating aligned samples.

Out of scope: non-auditable warping or mutation of imported source timestamps.

### Step 5 — Explain residuals by phase and evidence

Build phase-aware reconciliation for rail, powered ascent, coast, descent, and recovery across every available 6-DOF state, sensor, estimator, environment, configuration, and event channel. Report frame- and time-explicit event deltas, bias, MAE, RMSE, maximum absolute residual, attitude error, normalized error with an explicit denominator, ensemble-band coverage, completeness, and alignment confidence. Add deterministic diagnostic attribution by comparing residual shape against named model assumptions and controlled sensitivity reruns; label candidates as hypotheses, never proven causes.

Acceptance:

- Golden cases pin phase boundaries, residual series, metrics, uncertainty coverage, and the ranking/evidence for diagnostic hypotheses.
- Scalar scores link to residual-versus-time views so localized disagreement cannot be hidden by a whole-flight average.
- Credibility and reports surface threshold failures, configuration mismatches, inadequate coverage, extrapolation, and waived criteria.

Out of scope: an LLM-authored causal claim, unconstrained parameter fitting, or silently averaging simulator disagreement.

## Internal gate B — Mission Review Cinema

### Step 6 — Make the semantic timeline the workbench spine

Build one global review controller over `ReviewClock` with scrub, play, pause, step, speed, range selection, and named camera presets. Synchronize viewport, plots, residuals, evidence, and a multi-lane event strip covering flight phases, limits, evidence coverage, residual excursions, and decisions. Selecting any event seeks every view and reveals its detector, confidence, and source evidence.

Acceptance:

- Every consumer resolves the same review instant across predicted and measured domains; no component owns an independent clock.
- Ignition, rail exit, burnout, separation, apogee, deployment, limit crossings, residual peaks, and landing seek to exact recorded event times.
- Playback is deterministic across pause/resume, speed changes, reverse seeks, missing samples, and end behavior; renderers dispatch zero commands.

Out of scope: editing scientific state from the timeline or interpolating across unrelated evidence hashes.

### Step 7 — Build a counterfactual proposal universe

Unify approval and ghost proposals into one cloned review universe. Apply a validated external-agent batch to a cloned document, rerun affected studies, and derive proposed geometry, trace, events, metrics, credibility, and report preview. Display baseline, measured flight, and proposal simultaneously: baseline subdued and solid, changed geometry translucent, consequences high-contrast across every synchronized view.

Acceptance:

- Preview leaves the canonical document and journal byte-identical and produces deterministic cloned results with explicit parent hashes.
- Geometry, CG/CP, structural margin, stability, apogee, residual, uncertainty, stale-study, and evidence deltas agree across semantic and visual views.
- Reject and Escape restore the complete baseline; approve dispatches the canonical batch exactly once and turns the accepted counterfactual into ordinary journaled state.

Out of scope: renderer-owned proposal state, silent auto-approval, embedded model providers, or freehand mutation outside the command seam.

### Step 8 — Turn the journal into campaign cinema

Replay journal prefixes as immutable engineering checkpoints on the same semantic timeline. Each checkpoint exposes author/source, intent, canonical commands, affected requirements, before/after metrics, evidence hashes, and a branchable snapshot. Seeking uses deterministic checkpoints; restoring an earlier state creates a new journal decision rather than deleting history. Add a derived story track for camera and briefing beats that never changes scientific state.

Acceptance:

- Final cinema state is byte-identical to ordinary full replay; forward, backward, and checkpoint seeks equal replaying the same prefix from line zero.
- Corrupt journal lines stop exactly with actionable errors; playback never mutates or appends to the source journal.
- A golden campaign replays predict, import, align, reconcile, propose, preview, approve, and re-verify as one evidence-backed sequence.

Out of scope: invented intermediate engineering states, destructive history rewrite, lossy compaction, or presentation tracks that alter evidence.

## Internal gate C — Verifiable Release

### Step 9 — Export a deterministic mission-review bundle

Create a versioned review bundle containing manifest, source hashes, traces, alignment artifact, events, residual metrics, credibility cards, report-model JSON, standalone HTML, normalized PDF, and optional presentation track. Add dimensioned fin templates and longitudinal CG/CP layout. The bundle must reopen offline at the exact saved review state.

Acceptance:

- Identical scientific inputs produce byte-identical manifest, traces, metrics, report model, and HTML; PDF determinism is evaluated after explicitly documented metadata normalization.
- Every number and visual links to a study or evidence hash, alignment version, event detector version, and rendering settings.
- Stale, missing, extrapolated, waived, or low-confidence evidence remains visibly qualified in the bundle and reopened review.

Out of scope: a general-purpose document editor, runtime web assets, or unsupported claims of byte-identical platform print engines.

### Step 10 — Ship a cryptographically verifiable release train

Build native macOS and Windows release pipelines with pinned toolchains, protected signing environments, notarization/timestamping, independently signed Tauri updates, clean-machine install/update smoke tests, SHA-256 manifests, SPDX or CycloneDX SBOMs, hosted build-provenance attestations, and version-matched offline documentation bundled with the app.

Acceptance:

- A protected tag produces notarized macOS and signed Windows artifacts that install, launch, open the golden review bundle, and update on clean machines.
- Each artifact publishes checksum, SBOM, provenance attestation, app/schema/docs versions, and verification instructions; signing secrets are unavailable to fork and pull-request jobs.
- Unsigned payload inputs are repeatable where platform tooling permits; signed installers are verified by provenance and platform signatures rather than falsely claimed byte-identical.

Out of scope: SLSA Level 3 claims without a hardened-build audit, HSM infrastructure, delta updates, rollout telemetry, an extension marketplace, or automatic publishing from unreviewed branches.

## Headline demonstration

The v0.6 demo is one continuous technical review:

1. Open a predicted 6-DOF flight and scrub to rail exit.
2. Import a multi-source avionics telemetry and estimator bundle without changing its raw bytes.
3. Inspect frames, calibration, uncertainty, and clock domains, then approve the measured-to-simulation alignment.
4. Watch predicted truth, measured sensors, state estimates, and uncertainty envelopes separate through the flight.
5. Select the largest powered-ascent attitude or trajectory residual and inspect evidence-backed diagnostic hypotheses.
6. Ask an external agent for a correction and enter a cloned counterfactual review universe.
7. Scrub the proposed geometry and flight beside baseline and measured truth.
8. Approve once through the canonical command seam and re-run validation.
9. Replay the entire campaign as journal cinema.
10. Export the exact review state and verify the signed release artifact that produced it.

## Release gate

v0.6 closes only when all three internal gates pass; `cargo xtask test` is green from a clean release snapshot; canonical serialization, journal replay, validation, alignment, residual, counterfactual, and cinema goldens are deterministic; every bundled fixture has audited provenance and license; the review bundle reopens offline; native macOS and Windows smoke installs pass; artifact checksum, SBOM, provenance, updater signature, and offline docs agree with the tagged source; and no uncommitted release changes remain.
