# Evidence ladder — four distinct kinds of confidence

Ascent keeps four ideas separate on purpose. They are often collapsed into the
word "validated"; here they are not. Each answers a different question, and a
result can satisfy one while failing another.

| Concept | Question it answers | Where it lives |
|---|---|---|
| **Determinism** | Do identical inputs always produce identical bytes? | [`SIXDOF_DERIVATION.md`](SIXDOF_DERIVATION.md) (§ determinism rules), canonical-hash tests |
| **Verification** | Does the code solve its own equations correctly? | Unit/property tests, [`EVIDENCE.md`](EVIDENCE.md) regression against a frozen reference |
| **Cross-validation** | Does the model agree with an *independent* solver on a shared configuration? | [`CROSS_VALIDATION.md`](CROSS_VALIDATION.md) (RocketPy comparison seam) |
| **Real-flight validation** | Do the model's numbers match *measured flight data* within predefined tolerances? | An executed comparison artifact bound to a validation case (see below) |

Determinism is not correctness. Verification is not agreement with reality.
Cross-validation is agreement with another *model*, which can share the same
blind spots. Only real-flight validation compares against a measured flight —
and only for the specific quantities, vehicle, and conditions the flight covers.

## The rung ladder

The `EvidenceLevel` enum (`crates/ascent-domain/src/evidence.rs`) is the machine
encoding of this separation. The rungs run weakest to strongest, and each names
a *distinct kind* of evidence rather than "more testing":

1. **`Analytic`** — agrees with a closed-form solution.
2. **`UnitVerified`** — a unit/property test pins the behavior.
3. **`RegressionCompatible`** — reproduces another tool's export within a frozen
   tolerance. This is *compatibility*, not correctness.
4. **`CrossValidated`** — agrees with an independent engine (e.g. RocketPy) on a
   shared configuration.
5. **`FlightDataAvailable`** — real measured flight data is present,
   license-clear and hash-pinned, and the review plumbing ingests it, **but the
   model's numerical outputs have not yet been compared against it.** This rung
   is a claim about *data availability*, never about model accuracy.
6. **`FlightValidated`** — the model was executed and its numerical outputs were
   compared against measured flight data, and the comparison **passed**
   predefined tolerances.

The variants are deliberately **not** ordered (`Ord` is not derived): moving a
case up a rung is an evidence-gated act, never an automatic "greater-than".

## The `FlightValidated` invariant

A case may carry `FlightValidated` **only** if it also carries an executed
comparison artifact. This is enforced at load time, not by convention:
`ValidationCase::enforce_evidence_policy`
(`crates/ascent-review/src/validation.rs`) refuses to load a `FlightValidated`
case unless its `ComparisonArtifact` is present, internally consistent, bound to
that exact case definition (`case_hash`), and passing. The artifact records:

- the compared **metrics** (measured value, simulated value, absolute and
  relative error);
- the **tolerance** decided for each metric *before* the result was seen;
- the per-metric and overall **pass/fail**;
- the **source hashes** of every input consumed (measured fixture, model, config);
- the **model version** and canonical **input hash**;
- the **known limitations** of the comparison.

`ComparisonArtifact::validate` re-derives every error and pass/fail from the
stored measured/simulated numbers, so an artifact cannot be hand-edited to fake
a green result. A *failing* comparison is publishable — but only at a lower
rung; it can never wear the `FlightValidated` label.

The invariant is covered by executed tests in
`crates/ascent-review/src/validation.rs` (`evidence_policy_tests`): a
`FlightValidated` case cannot load without an artifact, loads with a valid
passing one, is refused with a failing one, is refused when the artifact is
bound to a different case, and an artifact cannot claim a pass its own numbers
do not support.

## Current status of the checked-in cases

| Case | Rung | What it establishes |
|---|---|---|
| `analytic-constant-thrust.json` | `Analytic` | Matches a closed-form constant-thrust solution. |
| `openrocket-24_12.json` | `RegressionCompatible` | Reproduces a frozen OpenRocket 24.12 export within tolerance. |
| `rocketpy-cross-validation.json` | `CrossValidated` | Agrees with RocketPy on a shared vertical configuration. |
| `valkyrie-2025-flight.json` | `FlightDataAvailable` | License-cleared measured Valkyrie 2025 flight data is ingested and hash-pinned. **No model-versus-flight numerical comparison has been executed yet.** |

**No case is `FlightValidated` today.** The Valkyrie case holds measured flight
data whose provenance is frozen, but a model-versus-flight comparison **cannot
currently be executed**: the checked-in dataset is a two-column altitude trace
with no simulation inputs (no vehicle, motor, launch geometry, or wind), so
there is nothing to run the model from. The full input/output inventory and the
reasons the inputs are not reconstructed from another tool are recorded in
[`VALKYRIE_VALIDATION.md`](VALKYRIE_VALIDATION.md). Until a documented,
hash-pinned flight configuration exists and an executed comparison passes
predefined tolerances, the case remains at `FlightDataAvailable`.
