# v0.6 Mission Review Cinema — research synthesis

*Generated 2026-07-19. Confidence: high on architecture and platform constraints; medium on public-flight fixture availability until each dataset's license and provenance are audited.*

## Executive summary

The ambitious move is not to bolt cinematic playback onto a simulator. It is to make time, evidence, and decisions one coherent substrate. NASA's modeling-and-simulation credibility standard emphasizes intended use, validation evidence, uncertainty, input pedigree, and explicit caveats. RocketPy establishes a useful comparison floor through imported flight data, common-grid interpolation, residual plots, and aggregate error metrics. NASA Open MCT, ParaView, and Basilisk Vizard show that serious review tools coordinate many views through one time authority. Figma and Blender contribute useful interaction patterns for named history checkpoints and simultaneous before/after states. Tauri, Apple, Microsoft, SLSA, SPDX, and CycloneDX show that release integrity should mean verifiable provenance and platform-native signing, not the false promise of byte-identical signed installers.

The resulting product concept is **Mission Review Cinema**: Ascent shows what was predicted, what actually happened, where and why they diverged, what a proposed change would do, and the complete evidence trail behind the decision. The cinema is not decoration. It is a synchronized projection of the same evidence graph used for metrics, credibility, approval, and reports.

## Vision boundary

Ascent is not a student, hobby, or college-competition product, and it is not being built to sell. OpenRocket compatibility is useful only as a low-fidelity regression reference. The architectural and credibility bar is the professional new-space stack: Basilisk's modular, repeatable simulation architecture; Nyx's validation discipline and flight heritage; RocketPy's atmospheric-rocket physics and flight comparison; GMAT-style named-baseline rigor; and mission-control/scientific-visualization review surfaces.

The intended user is a GNC or flight-dynamics engineer at a new-space or defense-tech startup. Accordingly, consumer altimeter CSVs are compatibility fixtures, not the canonical product model. The professional contract is a multi-source telemetry bundle with explicit time systems, coordinate frames, calibration, units, sensor uncertainty, estimator provenance, validity masks, and channels such as IMU, GNSS, barometry, state estimates, actuator/deployment state, and discrete events. The official comparison target is full 6-DOF state and uncertainty wherever evidence exists, not primarily an altitude curve.

## 1. Credibility must be evidence-shaped

NASA-STD-7009B establishes uniform modeling-and-simulation practices and explicitly addresses credibility products across development and use. Its companion handbook describes a validation-evidence ladder ranging from sanity checks and higher-fidelity simulations to experimental problems and real-system data. It also warns against unqualified point estimates and expects uncertainty, input pedigree, coverage, and caveats to remain visible ([NASA-STD-7009](https://standards.nasa.gov/standard/NASA/NASA-STD-7009), [NASA-HDBK-7009](https://ntrs.nasa.gov/citations/20140002378)).

Ascent should therefore replace a single pass/fail validation badge with an evidence card containing:

- intended use and validity domain;
- evidence level and source authority;
- frozen acceptance thresholds;
- estimate plus uncertainty or ensemble coverage;
- input pedigree and immutable source hashes;
- configuration mismatches, coverage gaps, and waived criteria.

## 2. Reconciliation should exceed the RocketPy baseline

RocketPy's `FlightComparator` accepts flight, avionics, simulator, and theoretical data; maps different sample rates to a common grid; plots residuals; and computes MAE, RMSE, maximum deviation, relative error, and key flight-event comparisons. Its importer maps external columns and units into SI. This is a credible interoperability floor, not the professional ceiling ([RocketPy Flight Comparator](https://docs.rocketpy.org/en/latest/user/flight_comparator.html), [FlightComparator source](https://github.com/RocketPy-Team/RocketPy/blob/master/rocketpy/simulation/flight_comparator.py), [FlightDataImporter source](https://github.com/RocketPy-Team/RocketPy/blob/master/rocketpy/simulation/flight_data_importer.py)).

Ascent should go further:

- preserve raw device clocks and bytes permanently;
- make manual, detected, and optimized time alignment explicit artifacts;
- store alignment algorithm, parameters, confidence, and objective;
- resample only inside valid overlap and retain missing-data masks;
- score rail, powered ascent, coast, descent, and recovery separately;
- report event deltas, bias, MAE, RMSE, maximum residual, ensemble coverage, completeness, and alignment confidence;
- prohibit unconstrained dynamic time warping from official scores because it can hide physics error;
- freeze thresholds in the case manifest before evaluation.

The canonical reconciliation model must extend beyond scalar altitude into position, velocity, attitude quaternion, body rates, acceleration/specific force, atmospheric state, mass/configuration events, recovery or actuator state, and estimator outputs when those channels exist. Metrics must name their coordinate frame and time basis. Device-specific CSV adapters live at the edge and map into this professional telemetry contract.

## 3. One playhead should govern every engineering view

NASA Open MCT brings historical and streaming telemetry, imagery, timelines, procedures, and other visualizations into one operational display. ParaView's Time Manager combines multiple temporal sources under one scene-time cursor. Basilisk Vizard supports recorded playback, live streams, scrubbing, playback speed, camera control, and visualization of modeled device states ([NASA Open MCT](https://nasa.github.io/openmct/), [ParaView Time Manager](https://docs.paraview.org/en/latest/UsersGuide/animation.html#time-manager), [Basilisk Vizard](https://hanspeterschaub.info/basilisk/Vizard/Vizard.html)).

Ascent needs a `ReviewClock`, not independent component timers. It must map three time domains without conflating them:

- simulation time;
- measured-flight time after an explicit alignment transform;
- journal/campaign time.

The viewport, plots, telemetry residuals, event lanes, evidence drawer, camera, and reports consume the same resolved review instant. Cameras may dramatize presentation; scientific state may never be invented between unsupported samples.

## 4. The timeline should expose causes and decisions

ParaView distinguishes temporal data tracks from presentation tracks and recomputes the pipeline at the chosen time. Figma provides named, inspectable version-history states. Blender onion skinning keeps before and after simultaneously legible with distinct colors and bounded ranges ([ParaView animation](https://docs.paraview.org/en/latest/UsersGuide/animation.html), [Figma version history](https://help.figma.com/hc/en-us/articles/360038006754-View-a-file-s-version-history), [Blender onion skinning](https://docs.blender.org/manual/en/latest/grease_pencil/properties/onion_skinning.html)).

Ascent's semantic timeline should contain typed lanes for:

- flight phases and detected events;
- structural, stability, recovery, and mission limits;
- simulated and measured evidence coverage;
- residual excursions and diagnostic hypotheses;
- proposals, previews, approvals, and journal commits;
- derived camera and briefing beats.

Every event records detector version, source evidence, confidence, and affected views. Selecting an event seeks the whole workbench and opens its governing evidence. History restoration creates a new journal decision rather than erasing provenance.

## 5. Counterfactual review is the category-defining interaction

A pending proposal should instantiate a complete cloned review universe: cloned document, recomputed studies, trace, metrics, events, evidence, and report preview. The baseline stays solid and subdued; changed geometry is translucent; propagated consequences receive high-contrast analytical overlays. The canonical document and journal remain byte-identical until one explicit approval dispatch.

This enables the headline sequence:

1. Review predicted flight.
2. Overlay measured telemetry.
3. Seek the largest phase-local residual.
4. Inspect evidence-backed diagnostic candidates.
5. Preview an agent's correction as counterfactual geometry and flight.
6. Scrub baseline, measured, and proposed states under one playhead.
7. Approve once through the command seam.
8. Replay the complete campaign as a provenance-preserving engineering story.

## 6. Distribution should be cryptographically verifiable

Tauri requires platform-native macOS and Windows signing flows; its updater uses a separate application update key. Apple notarization and trusted timestamps, and Windows timestamping and reputation systems, add external and time-dependent material. Signed installers should therefore not be promised as byte-identical ([Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/), [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution), [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/), [Microsoft Smart App Control signing](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control), [Tauri updater](https://v2.tauri.app/plugin/updater/)).

The v0.6 claim should instead be verifiable, repeatable, platform-native releases:

- reproducible unsigned payload inputs where platform tooling permits;
- notarized macOS and signed Windows artifacts on native CI runners;
- SHA-256 manifest, SPDX or CycloneDX SBOM, and signed provenance attestation per artifact;
- independent updater signature, staged channel, rotation plan, and rollback drill;
- version-matched offline documentation bundled with the app;
- verification instructions for platform signatures, hashes, updater signature, SBOM, and provenance.

SLSA Build L2-style hosted provenance is a realistic v0.6 target; a full L3 claim requires a separate hardened-build audit ([SLSA levels](https://slsa.dev/spec/v1.1/levels), [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations), [SPDX](https://spdx.dev/use/specifications/), [CycloneDX](https://cyclonedx.org/specification/overview/), [Reproducible Builds](https://reproducible-builds.org/docs/source-date-epoch/)).

## Product conclusion

v0.6 should be one release with three internal gates:

- **v0.6A Evidence Spine:** canonical traces, evidence manifests, validation, immutable flight ingest, explicit alignment, and phase-aware reconciliation.
- **v0.6B Mission Review Cinema:** global review clock, semantic event lanes, synchronized predicted/measured/counterfactual playback, approval, and campaign replay.
- **v0.6C Verifiable Release:** deterministic review bundle, signed platform artifacts, updater trust, SBOM, provenance, and matching offline documentation.

The release claim is:

> Ascent shows what you predicted, what actually happened, why they differed, what change would improve it, and the complete evidence trail behind the decision.

## Methodology and limitations

The research investigated five questions: credibility standards, flight-data comparison, synchronized engineering review, provenance/history interaction, and desktop release integrity. Primary official sources were read directly. Exa and Firecrawl were unavailable in this runtime, so source breadth is narrower than the deep-research skill's preferred 15–30-source sweep. Claims that depend on candidate public-flight datasets remain conditional on a fixture-by-fixture license and provenance audit.
