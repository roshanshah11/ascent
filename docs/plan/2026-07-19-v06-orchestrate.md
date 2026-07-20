# Plan-Orchestrate Result

**Plan**: `docs/plan/2026-07-19-v06-closed-loop-cinematic.md`
**Lang**: `unknown` (Rust/TypeScript polyglot)
**ECC mode**: `plugin (local ecc namespace)`
**Steps**: 10
**Scope**: `all`

## Steps overview

| # | Title | Tags | Chain |
|---|---|---|---|
| 1 | Establish the temporal evidence kernel | design, impl, test | `ecc:planner,ecc:architect,ecc:tdd-guide,ecc:code-reviewer` |
| 2 | Productize the validation evidence ladder | impl, test, review | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 3 | Ingest professional telemetry as immutable evidence | impl, test, migration | `ecc:architect,ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 4 | Align clocks without hiding physics error | impl, test | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 5 | Explain residuals by phase and evidence | impl, test, review | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 6 | Make the semantic timeline the workbench spine | impl, test, review | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 7 | Build a counterfactual proposal universe | impl, test, review | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 8 | Turn the journal into campaign cinema | impl, test | `ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer` |
| 9 | Export a deterministic mission-review bundle | impl, test, docs | `ecc:tdd-guide,ecc:e2e-runner,ecc:doc-updater,ecc:code-reviewer` |
| 10 | Ship a cryptographically verifiable release train | impl, test, security, build, docs | `ecc:tdd-guide,ecc:e2e-runner,ecc:build-error-resolver,ecc:security-reviewer` |

---

## Step 1 — Establish the temporal evidence kernel

**Intent**: Design the versioned `FlightTrace`, `TelemetryBundle`, `EvidenceManifest`, typed-event, and three-domain `ReviewClock` contracts that every later v0.6 surface consumes.
**Tags**: design, impl, test
**Chain rationale**: `ecc:planner` and `ecc:architect` establish the cross-crate contracts, `ecc:tdd-guide` implements from serialization invariants, and `ecc:code-reviewer` closes the polyglot boundary review.

```bash
/ecc:orchestrate custom "ecc:planner,ecc:architect,ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-1] Design FlightTrace, TelemetryBundle, EvidenceManifest, typed events, and ReviewClock for 6-DOF truth, sensors, estimates, uncertainty, frames, and multiple clocks while separating raw, calibrated, simulated, derived, and presentation tracks; Acceptance: canonical bytes and contracts are validated; source clocks remain untouched; existing numerical goldens do not change; Out of scope: live telemetry transport, arbitrary user scripting, or a second persistence system."
```

## Step 2 — Productize the validation evidence ladder

**Intent**: Turn named simulator and real-flight fixtures into intended-use validation cases with explicit evidence strength, pedigree, uncertainty, thresholds, and caveats.
**Tags**: impl, test, review
**Chain rationale**: `ecc:tdd-guide` pins the manifest contract, `ecc:e2e-runner` proves complete offline cases, and `ecc:code-reviewer` audits credibility and hash flow.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-2] Build the NASA-style validation ladder from analytic truth through OpenRocket compatibility and RocketPy cross-validation to one licensed full-scale flight, with intended use, pedigree, uncertainty, validity, frozen thresholds, and caveats; Acceptance: manifests are provenance-complete; changes invalidate results; qualifications stay visible; Out of scope: runtime downloads, undisclosed fixture transformations, or accuracy claims outside a case's declared validity domain."
```

## Step 3 — Ingest professional telemetry as immutable evidence

**Intent**: Ingest multi-source avionics telemetry and estimator outputs while preserving raw bytes, clock domains, frames, calibration, uncertainty, validity, and transformation lineage.
**Tags**: impl, test, migration
**Chain rationale**: `ecc:architect` protects the format migration, `ecc:tdd-guide` drives importer behavior, `ecc:e2e-runner` exercises all devices, and `ecc:code-reviewer` closes the evidence-boundary review.

```bash
/ecc:orchestrate custom "ecc:architect,ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-3] Build schema-driven ingestion for multi-rate IMU, GNSS, barometry, estimators, environment, configuration, and events, with consumer-device adapters only as edge fixtures; Acceptance: raw bytes, clocks, frames, calibration, units, uncertainty, validity, mappings, and parent hashes persist; imports and replay are deterministic; Out of scope: cloud sync, live radio ingest, a hardware-driver ecosystem, or automatic device detection without a recorded decision."
```

## Step 4 — Align clocks without hiding physics error

**Intent**: Make relationships among simulation, avionics, sensor, estimator, and vehicle clock domains inspectable evidence rather than invisible preprocessing.
**Tags**: impl, test
**Chain rationale**: `ecc:tdd-guide` pins alignment mathematics and failure modes, `ecc:e2e-runner` validates synthetic and device traces, and `ecc:code-reviewer` protects scientific honesty.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-4] Create auditable manual, event-correlated, and constrained offset/drift relationships across simulation, sensor, estimator, and vehicle clocks, recording time systems, discontinuities, overlap, objective, and confidence; Acceptance: synthetic fixtures recover frozen truth; modes compare before journaled selection; weak evidence fails visibly; Out of scope: non-auditable warping or mutation of imported source timestamps."
```

## Step 5 — Explain residuals by phase and evidence

**Intent**: Reconcile full predicted 6-DOF truth, measured sensors, and estimator outputs by physical phase and produce evidence-backed diagnostic hypotheses instead of one opaque score.
**Tags**: impl, test, review
**Chain rationale**: `ecc:tdd-guide` pins metrics and attribution, `ecc:e2e-runner` proves full reconciliations, and `ecc:code-reviewer` reviews uncertainty and non-causal labeling.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-5] Build frame- and time-explicit phase reconciliation across position, velocity, attitude, rates, acceleration, environment, configuration, sensors, estimates, and events, plus uncertainty coverage and sensitivity-ranked hypotheses; Acceptance: golden 6-DOF residuals, metrics, coverage, and rankings are pinned and linked to time views; Out of scope: an LLM-authored causal claim, unconstrained parameter fitting, or silently averaging simulator disagreement."
```

## Step 6 — Make the semantic timeline the workbench spine

**Intent**: Synchronize predicted flight, measured evidence, residuals, limits, decisions, viewport, plots, and cameras through one review controller.
**Tags**: impl, test, review
**Chain rationale**: `ecc:tdd-guide` specifies playback state, `ecc:e2e-runner` validates synchronized views, and `ecc:code-reviewer` gates clock ownership and read-only rendering.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-6] Build one ReviewClock controller with scrub, play, pause, step, speed, range, camera presets, and typed event lanes synchronized across viewport, plots, residuals, and evidence; Acceptance: all consumers resolve one instant; named events seek exactly with detector evidence; playback is deterministic and renderers dispatch zero commands; Out of scope: editing scientific state from the timeline or interpolating across unrelated evidence hashes."
```

## Step 7 — Build a counterfactual proposal universe

**Intent**: Preview an external agent's proposed design and complete recomputed flight-review consequences without mutating canonical state.
**Tags**: impl, test, review
**Chain rationale**: `ecc:tdd-guide` enforces cloned-state semantics, `ecc:e2e-runner` covers propose-preview-approve flows, and `ecc:code-reviewer` protects the single dispatcher.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-7] Build one cloned counterfactual review universe deriving proposed geometry, studies, trace, events, metrics, credibility, and report beside baseline and measured truth; Acceptance: preview is deterministic and byte-preserving; all semantic and visual deltas agree; reject fully restores and approve dispatches one canonical batch; Out of scope: renderer-owned proposal state, silent auto-approval, embedded model providers, or freehand mutation outside the command seam."
```

## Step 8 — Turn the journal into campaign cinema

**Intent**: Replay the full engineering campaign as inspectable, seekable checkpoints on the same evidence-bound semantic timeline.
**Tags**: impl, test
**Chain rationale**: `ecc:tdd-guide` pins prefix/checkpoint equivalence, `ecc:e2e-runner` exercises a complete campaign, and `ecc:code-reviewer` reviews immutability and provenance.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-8] Implement journal cinema with immutable checkpoints exposing intent, commands, requirements, before/after metrics, evidence hashes, branchable snapshots, and derived story tracks; Acceptance: final and seeked states equal ordinary prefix replay byte-for-byte; corrupt lines stop exactly; a golden campaign covers predict through re-verify; Out of scope: invented intermediate engineering states, destructive history rewrite, lossy compaction, or presentation tracks that alter evidence."
```

## Step 9 — Export a deterministic mission-review bundle

**Intent**: Package the exact scientific and presentation state of a review into a reopenable offline bundle and engineering briefing.
**Tags**: impl, test, docs
**Chain rationale**: `ecc:tdd-guide` pins bundle determinism, `ecc:e2e-runner` proves offline reopen, `ecc:doc-updater` validates the format contract, and `ecc:code-reviewer` closes provenance review.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:doc-updater,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-9] Export a versioned offline review bundle with hashes, traces, alignment, events, residuals, credibility, report JSON/HTML/PDF, fin templates, CG/CP layout, and optional story track; Acceptance: deterministic fields are byte-identical; every value links to evidence and versions; reopening restores exact review state and qualifications; Out of scope: a general-purpose document editor, runtime web assets, or unsupported claims of byte-identical platform print engines."
```

## Step 10 — Ship a cryptographically verifiable release train

**Intent**: Produce protected, signed, attestable, updateable native releases whose inputs and evidence can be independently verified.
**Tags**: impl, test, security, build, docs
**Chain rationale**: `ecc:tdd-guide` defines release behavior, `ecc:e2e-runner` runs native smoke flows, `ecc:build-error-resolver` handles platform failures, and `ecc:security-reviewer` closes signing and updater trust.

```bash
/ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:build-error-resolver,ecc:security-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-10] Build protected macOS/Windows pipelines with notarized/signed apps, signed updates, clean-machine smoke tests, checksums, SBOMs, provenance attestations, and matching offline docs; Acceptance: tagged artifacts install, launch, open the golden bundle, and update; verification metadata agrees; secrets never reach untrusted jobs; Out of scope: SLSA Level 3 claims without a hardened-build audit, HSM infrastructure, delta updates, rollout telemetry, an extension marketplace, or automatic publishing from unreviewed branches."
```

## Batch execution

```bash
$ecc:orchestrate custom "ecc:planner,ecc:architect,ecc:tdd-guide,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-1] Design FlightTrace, TelemetryBundle, EvidenceManifest, typed events, and ReviewClock for 6-DOF truth, sensors, estimates, uncertainty, frames, and multiple clocks while separating raw, calibrated, simulated, derived, and presentation tracks; Acceptance: canonical bytes and contracts are validated; source clocks remain untouched; existing numerical goldens do not change; Out of scope: live telemetry transport, arbitrary user scripting, or a second persistence system."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-2] Build the NASA-style validation ladder from analytic truth through OpenRocket compatibility and RocketPy cross-validation to one licensed full-scale flight, with intended use, pedigree, uncertainty, validity, frozen thresholds, and caveats; Acceptance: manifests are provenance-complete; changes invalidate results; qualifications stay visible; Out of scope: runtime downloads, undisclosed fixture transformations, or accuracy claims outside a case's declared validity domain."
$ecc:orchestrate custom "ecc:architect,ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-3] Build schema-driven ingestion for multi-rate IMU, GNSS, barometry, estimators, environment, configuration, and events, with consumer-device adapters only as edge fixtures; Acceptance: raw bytes, clocks, frames, calibration, units, uncertainty, validity, mappings, and parent hashes persist; imports and replay are deterministic; Out of scope: cloud sync, live radio ingest, a hardware-driver ecosystem, or automatic device detection without a recorded decision."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-4] Create auditable manual, event-correlated, and constrained offset/drift relationships across simulation, sensor, estimator, and vehicle clocks, recording time systems, discontinuities, overlap, objective, and confidence; Acceptance: synthetic fixtures recover frozen truth; modes compare before journaled selection; weak evidence fails visibly; Out of scope: non-auditable warping or mutation of imported source timestamps."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-5] Build frame- and time-explicit phase reconciliation across position, velocity, attitude, rates, acceleration, environment, configuration, sensors, estimates, and events, plus uncertainty coverage and sensitivity-ranked hypotheses; Acceptance: golden 6-DOF residuals, metrics, coverage, and rankings are pinned and linked to time views; Out of scope: an LLM-authored causal claim, unconstrained parameter fitting, or silently averaging simulator disagreement."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-6] Build one ReviewClock controller with scrub, play, pause, step, speed, range, camera presets, and typed event lanes synchronized across viewport, plots, residuals, and evidence; Acceptance: all consumers resolve one instant; named events seek exactly with detector evidence; playback is deterministic and renderers dispatch zero commands; Out of scope: editing scientific state from the timeline or interpolating across unrelated evidence hashes."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-7] Build one cloned counterfactual review universe deriving proposed geometry, studies, trace, events, metrics, credibility, and report beside baseline and measured truth; Acceptance: preview is deterministic and byte-preserving; all semantic and visual deltas agree; reject fully restores and approve dispatches one canonical batch; Out of scope: renderer-owned proposal state, silent auto-approval, embedded model providers, or freehand mutation outside the command seam."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-8] Implement journal cinema with immutable checkpoints exposing intent, commands, requirements, before/after metrics, evidence hashes, branchable snapshots, and derived story tracks; Acceptance: final and seeked states equal ordinary prefix replay byte-for-byte; corrupt lines stop exactly; a golden campaign covers predict through re-verify; Out of scope: invented intermediate engineering states, destructive history rewrite, lossy compaction, or presentation tracks that alter evidence."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:doc-updater,ecc:code-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-9] Export a versioned offline review bundle with hashes, traces, alignment, events, residuals, credibility, report JSON/HTML/PDF, fin templates, CG/CP layout, and optional story track; Acceptance: deterministic fields are byte-identical; every value links to evidence and versions; reopening restores exact review state and qualifications; Out of scope: a general-purpose document editor, runtime web assets, or unsupported claims of byte-identical platform print engines."
$ecc:orchestrate custom "ecc:tdd-guide,ecc:e2e-runner,ecc:build-error-resolver,ecc:security-reviewer" "[Plan: docs/plan/2026-07-19-v06-closed-loop-cinematic.md#step-10] Build protected macOS/Windows pipelines with notarized/signed apps, signed updates, clean-machine smoke tests, checksums, SBOMs, provenance attestations, and matching offline docs; Acceptance: tagged artifacts install, launch, open the golden bundle, and update; verification metadata agrees; secrets never reach untrusted jobs; Out of scope: SLSA Level 3 claims without a hardened-build audit, HSM infrastructure, delta updates, rollout telemetry, an extension marketplace, or automatic publishing from unreviewed branches."
```
