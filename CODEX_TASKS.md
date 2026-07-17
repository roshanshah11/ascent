# Codex Task Prompts — Ascent v0.1

Each block below is a complete, self-contained prompt: paste it into Codex verbatim, working directory `~/Projects/ascent`. Every prompt names its deliverable paths and its boundary.

**Coordination rule (prevents agent collisions):** Track A tasks only create NEW files — safe to run anytime, in parallel with Claude's build. Track B tasks own the whole repo — run at most one at a time, and only when Claude is not mid-build-day. When in doubt, run Track A.

---

## TRACK A — parallel-safe (new files only; run any of these now)

### A1. IREC 2026 rule pack (highest value — Day 7 consumes it directly)

> Read docs/FULL_BUILD_RESEARCH.md sections "Competition Flight Review" and "IREC rule-pack JSON schema". From the primary sources — IREC 2026 Rules and Requirements Document and the 2026 IREC DTEG (both linked in docs/FULL_BUILD_RESEARCH.md §Sources) — extract every numeric flight-performance constraint into `data/rules/irec-2026.json` following that schema. Include per rule: id, description, quantity, comparator, value, units, applicability (10k/30k/45k category, COTS/SRAD), and a citation object with document name, version, and section number. Cover at minimum: rail-departure velocity, stability margin bands (subsonic/supersonic/launch/flight, percent-of-length form AND caliber form if both appear), fin span minimum, target apogees, and any recovery-deployment requirements with numbers. Also write `data/rules/README.md` explaining schema and provenance. Do NOT modify any existing file. Acceptance: JSON parses; every rule has a citation with a section number; no value asserted without a source.

### A2. Barrowman worksheet + hand-computed stability fixture (feeds Day 4 tests)

> From the OpenRocket technical documentation (openrocket.sourceforge.net/techdoc.pdf) §3, write `docs/BARROWMAN_WORKSHEET.md`: the exact Barrowman equations Ascent v0.1 needs for a single-stage rocket — nose cone CNα and CP, conical transition, trapezoidal fin set (including body-interference factor), and the CP-combination formula. State each equation, its symbols, units, and validity limits (small AoA, subsonic). Then hand-compute a full worked example for the Estes Alpha III (get real dimensions from Estes/OpenRocket's bundled alpha3.ork geometry: nose length, body diameter/length, fin root/tip/span/sweep) and write the result to `data/fixtures/alpha3_stability.json`: each component's CNα and CP position, the total CP from reference datum, and expected CG with a loaded C6. Show all arithmetic in the worksheet so the numbers are auditable. Do NOT modify any existing file. Acceptance: worked example arithmetic checks; fixture JSON has component-level and total values; every equation cites a techdoc section.

### A3. Drag model note + Cd table (feeds Day 3–4 accuracy)

> Write `docs/DRAG_MODEL.md`: a defensible drag model for a low-power model rocket (Alpha III class, Mach < 0.4). Cover: what constant-Cd misses, the standard subsonic Cd build-up (nose + body friction + base + fin + launch-lug), typical published Cd values for an Alpha III-class rocket with sources, and a recommended Mach-Cd table for v0.1 written to `data/aero/alpha3_cd.json` (Mach breakpoints 0 to 0.5, cd values, source + validity metadata per the fidelity-ladder rules in docs/IDEA_SPEC.md). Note current sim uses Cd 0.60 flat — state whether that is high/low and why (current sim result: apogee 361 m vertical, vs Estes advertised ~335 m). Do NOT modify any existing file. Acceptance: every Cd number has a named source; the JSON has source and validity fields; a one-paragraph recommendation says exactly what Day 3 should change, if anything.

### A4. Motor bundle: two more provenance-preserved motors

> Read crates/ascent-domain/data/motors/estes_c6.json — this is the exact schema and provenance pattern. Using the ThrustCurve.org API v1 (documented at thrustcurve.org/info/api.html; search.json to find motorId, download.json with format=RASP&data=samples to get points; prefer source=cert data), create `crates/ascent-domain/data/motors/estes_b6.json` (Estes B6) and `crates/ascent-domain/data/motors/estes_d12.json` (Estes D12), same fields: designation, manufacturer, provenance (source, cert_org, motor_id, info_url, license, retrieved date, format), diameter/length, total/propellant mass from certified metadata, expected impulse/burn/avg/max thrust, thrust_curve samples. Do NOT modify existing files or any Rust code. Acceptance: both JSONs match the C6 schema field-for-field; thrust curves end at zero thrust with strictly increasing times; masses and impulse come from the certified metadata, stated in the provenance.

### A5. 90-second demo script + shot list (Day 9 prep)

> Read docs/PROJECT_SPEC.md (§v0.1 and §demo) and docs/FULL_BUILD_RESEARCH.md §"The 90-second July 23 demo". Write `docs/DEMO_SCRIPT.md`: a timed 90-second script (beat by beat, with what is on screen and what Roshan says — plain spoken sentences, no startup clichés), a backup-video shot list (each shot, duration, what it proves), and a 3-line fallback narration if the live app fails. The demo arc: open a rule-violating IREC design → constraint panel shows red → "Find feasible configuration" repairs it → launch → evidence drawer proves the number. Do NOT modify any existing file. Acceptance: beats sum to ≤90 s; every claim in the script maps to a feature that exists in the Day 1–8 plan; zero unverifiable claims.

### A6. RASP .eng conformance corpus (feeds first-30-days import work)

> Write `docs/RASP_FORMAT.md`: the RASP .eng file format spec (header line fields and units, comment lines, data pairs, terminating zero-thrust convention, multi-motor files), citing the ThrustCurve RASP documentation page. Collect 5 real .eng files spanning the quirks (comments, multiple motors per file, missing final zero, header unit variations) into `data/eng-samples/` with a `data/eng-samples/MANIFEST.json` recording per file: where it came from, license note, and which quirk it exercises. Do NOT modify any existing file or any Rust code. Acceptance: spec covers every field in the header line; each sample file's quirk is named in the manifest; licenses recorded.

---

## TRACK B — build days (repo-owning; ONE at a time, coordinate with Claude)

### B3. Day 3 — OpenRocket golden fixture (BLOCKED until Roshan exports the OpenRocket numbers)

> Working in ~/Projects/ascent (Rust workspace; run tests with `cargo test`). Read docs/FULL_BUILD_RESEARCH.md §"Build sequence" Day 3 and CHECKLIST.md. A manual OpenRocket 24.12 export for Estes Alpha III on C6-5 exists at data/reference/openrocket-alpha3-c6.csv [Roshan: put the export here first]. Create `crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json` freezing OpenRocket's apogee, max velocity, burnout time/velocity, and flight time, with metadata (OpenRocket version, sim settings, export date). Add a regression test in crates/ascent-sim/tests/ comparing simulate_vertical's Alpha III output (constructor already in tests/flight.rs) against the fixture with explicit tolerances, and write `docs/EVIDENCE.md`: a table of what agrees, what differs, and why (our sim is vertical-only point-mass, constant Cd 0.60; OpenRocket flies 3D with Mach-dependent Cd). Acceptance: cargo test green including the new fixture test; every tolerance justified in EVIDENCE.md; no changes to sim physics unless EVIDENCE.md documents why.

### B4. Day 4 — parametric vehicle + Barrowman stability

> Working in ~/Projects/ascent (Rust workspace, crates: ascent-domain, ascent-sim; `cargo test` must stay green). Read docs/FULL_BUILD_RESEARCH.md §"Domain model" and §"Physics-core specification", docs/BARROWMAN_WORKSHEET.md, and data/fixtures/alpha3_stability.json (hand-computed expected values — these are your test fixtures). Create crate `crates/ascent-aero` (workspace member, depends on ascent-domain): parametric vehicle model (nose cone, body tube, trapezoidal fin set, payload mass, motor mount, chute), component masses and CG, Barrowman CP per the worksheet, and time-varying stability margin in calibers using motor mass_at(t) from ascent-domain. Then IREC constraint checks against data/rules/irec-2026.json: rail-exit velocity, stability bands, fin span, each check citing its rule id. Write tests first: component CP/CNα vs the hand-computed fixture, total CP, CG shift during burn, stability in calibers, constraint pass/fail at rule boundary values. Acceptance: fixture-derived tests green; editing fin span or adding payload mass changes CP/CG/stability in the physically correct direction (tested); whole workspace cargo test green.

### B5–B8. Days 5–8 (UI + solver) — do not start until B3/B4 merged

Prompts for the Tauri shell, flight mode, Flight Review + solver, and provenance/hardening will be cut from docs/FULL_BUILD_RESEARCH.md §Build sequence the same way once Day 4 lands — the UI contracts depend on what ascent-aero actually exposes. Ask Claude for them when B4 is done (or paste the Day 5–8 orchestrate lines from CHECKLIST.md into Claude).

---

## Suggested order for Codex right now

1. **A1** (rule pack) — Day 7 depends on it, zero collision risk.
2. **A2** (Barrowman worksheet + fixture) — Day 4 tests depend on it.
3. **A3** (drag note) then **A4** (motors) then **A5** (demo script) then **A6** (.eng corpus).
4. Track B only by explicit handoff.
