# Ascent

Ascent is a local-first aerospace engineering workbench for designing, simulating and reviewing rocket flights. The physics is a Rust workspace, the desktop app is Tauri v2 with React, and every result carries the inputs, models and assumptions that produced it. People and AI agents change a design through the same audited command layer, so every edit is journaled and replayable.

**Status:** v0.6.0 (July 2026). A personal project built to a professional bar, not a commercial product. It starts with sounding-rocket-class vehicles because that is where the validated physics and reference data exist today.

## What it does

- **Design.** A vehicle is a model tree (nose, body tube, fin sets, point masses, stage couplers), not a flat parameter list. Ascent computes Barrowman center of pressure, time-varying center of gravity and stability margin in calibers.
- **Simulate.** 1-DOF vertical, 3-DOF planar and 6-DOF engines, multi-stage flights with variable-mass handoff, the 1976 standard atmosphere, wind profiles, and seeded Monte Carlo dispersion with landing ellipses. Motors load from RASP `.eng` files with their certification provenance.
- **Review.** Requirement checks against versioned rule packs, each check citing its rule source. A deterministic repair solver enumerates motors and bisects ballast to move an infeasible design toward an apogee target without leaving its stability window, then shows the before and after diff.
- **Mission review (v0.6).** A semantic timeline you scrub through a flight, telemetry ingestion with clock alignment and phase-aware residual reconciliation, and exportable review bundles whose hashes can be verified offline.
- **Evidence on every run.** Input hash, model versions, stated assumptions, motor certification data and a live timestep-convergence check travel with each result.

## How it is checked

Physics claims are tested against named baselines, and the tolerances live in the test suite.

Against **OpenRocket 24.12** (its bundled "A simple model rocket" example on an Estes C6-5, matched configuration):

| Quantity | OpenRocket | Ascent | Difference |
|---|---|---|---|
| Burnout time | 1.860 s | 1.860 s | 0.0% |
| Max velocity | 95.3 m/s | 95.37 m/s | +0.1% |
| Apogee | 316.8 m | 321.2 m | +1.4% |
| Descent rate | 4.17 m/s | 4.12 m/s | -1.2% |

Every remaining difference is traced to a named modeling choice (vertical vs 3D flight in wind, deploy at apogee vs at the ejection charge). The full table and the reasoning are in [`docs/EVIDENCE.md`](docs/EVIDENCE.md).

- Total Barrowman center of pressure agrees within 0.5% of both a hand-derived fixture and OpenRocket's own implementation value (`crates/ascent-aero/tests/alpha3_fixture.rs`).
- A golden regression pin freezes the reference flight summary to 1e-9, and command journals replay byte for byte.
- An optional RocketPy bridge runs the same vehicle through RocketPy and reports the apogee delta next to Ascent's native result, without ever replacing it ([`docs/CROSS_VALIDATION.md`](docs/CROSS_VALIDATION.md)).

## How it is built

```
ascent-domain   motors + vehicle model tree           (no renderer, no UI, no engine)
      │
      ├── ascent-aero    Barrowman CP, CG, stability, rule-pack constraints
      │
      ├── ascent-sim     flight engines: 1-DOF / 3-DOF planar / 6-DOF / dispersion / staged
      │
      ├── ascent-review  requirements verification + repair solver + structural margins
      │
      └── ascent-app     command spine, Document, studies, jobs, evidence, DTOs, Tauri IPC, CLI
             │
             ├── ascent-mcp   thin MCP stdio adapter over propose/apply/run_study
             │
             └── app/         React + R3F frontend (snapshots-only)
```

Every change to a design is a serializable `Command` applied through one `Document` dispatcher, and each apply returns its inverse, which is how undo and redo work. The GUI, command palette, console, CLI and MCP server are all clients of that one dispatcher. None of them edits state directly. The frontend never simulates: it renders stored run records, and flight playback makes zero simulation calls. Nothing uses the network at runtime. Motor curves, soundings and logs come in as files and are hashed into each study's input hash.

AI never edits a design directly. It proposes typed commands, a person approves them, and they go through the same journal as any other edit ([`docs/COPILOT_INTERFACE.md`](docs/COPILOT_INTERFACE.md)).

The architecture and its invariants are documented in [`DESIGN.md`](DESIGN.md).

## Running it

**You need** Rust 1.97 or newer, Node 22, and the Tauri v2 CLI for the desktop app.

**Desktop app:**

```sh
git clone https://github.com/roshanshah11/ascent.git
cd ascent
npm --prefix app install
cargo install tauri-cli --locked
cd crates/ascent-app && cargo tauri dev
```

**Headless CLI:**

```sh
cargo run -p ascent-app --bin ascent-cli -- run-study <project.ascent> <study-id-or-name>
cargo run -p ascent-app --bin ascent-cli -- replay <session.jsonl>
```

`run-study` prints hash-stamped results JSON, and `replay` rebuilds a document from its command journal. To check the pipeline end to end, generate the golden review bundle and verify it:

```sh
cargo run -p ascent-app --bin ascent-cli -- create-review-bundle demo.ascent-review
cargo run -p ascent-app --bin ascent-cli -- verify-review-bundle demo.ascent-review
```

**MCP server for agents:** `cargo run -p ascent-mcp` starts a stdio server with `get_document`, `propose_commands`, `apply_proposal`, `run_study` and `read_evidence`.

**Tests:** `cargo xtask test` runs the full gate: formatting, clippy, MCP agent evals, the Rust workspace tests, the TypeScript typecheck, vitest and a production frontend build. CI also runs `cargo audit` and `cargo vet`.

## Where to read more

| Doc | What it covers |
|---|---|
| [`docs/PRODUCT_DIRECTION.md`](docs/PRODUCT_DIRECTION.md) | What Ascent is and is not |
| [`DESIGN.md`](DESIGN.md) | Architecture and the invariants every part keeps |
| [`docs/EVIDENCE.md`](docs/EVIDENCE.md) | The OpenRocket comparison, number by number |
| [`docs/JOURNAL_FORMAT.md`](docs/JOURNAL_FORMAT.md) | The command grammar every client speaks |
| [`docs/VEHICLE_TREE.md`](docs/VEHICLE_TREE.md) | Part kinds and their parameters |

## License

MIT, as declared in the workspace `Cargo.toml`.
