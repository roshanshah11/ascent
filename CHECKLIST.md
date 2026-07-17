# Ascent v0.1 — Master Checklist (through Jul 23 demo)

Plan source: `docs/FULL_BUILD_RESEARCH.md` §Build sequence.
Companion specs: `PROJECT_SPEC.md` (9-day slice), `IDEA_SPEC.md` (full idea).

## Build days (each has an exit gate — don't advance without it)

- [x] **Day 1 — trusted motor + vertical-flight kernel** *(DONE 2026-07-17, commit `1a4477c`)*
  Rust workspace, bundled C6 curve with provenance, interpolation/impulse/mass-depletion + tests, point-mass derivative, RK4, analytic no-drag fixture.
  Exit passed: CLI prints deterministic trajectory; all physics tests pass. Alpha III/C6 → 358 m.
- [x] **Day 2 — flight phases + convergence** *(DONE 2026-07-17, commit `95841a1`)*
  Rail constraint, drag, 1976 standard atmosphere, event detection (liftoff, rail exit, burnout, apogee, recovery deploy, landing), chute descent, timestep convergence report.
  Exit passed: full event timeline; apogee stable to <1 mm at dt/2; 38/38 tests.
- [x] **Day 3 — one external reference** *(DONE 2026-07-17)*
  Frozen fixture from manual OpenRocket 24.12 export ("simple model rocket" example / C6-5); regression tests with explicit tolerances; `docs/EVIDENCE.md` agree/differ/why table.
  Exit passed: apogee within 1.4%, max velocity 0.1%; every residual traced to a named modeling choice. 42/42 tests.
- [x] **Day 4 — parametric vehicle + stability** *(DONE 2026-07-17, commit `7632645`)*
  `ascent-aero` crate: nose/body/fins/point-mass model, time-varying CG, Barrowman CP (techdoc), stability in calibers, rule-pack constraint engine (plan defaults until A1 lands; A2 fixture adds exact Alpha III pins).
  Exit passed: span/payload/tail-mass edits produce correct tested stability changes; constraint pass/fail tested at rule boundaries. 57 tests total.
- [ ] **Day 5 — app shell + design mode** (Tauri + React)
  Viewport, inspector, motor selector; design revision + dirty/stale/running/current states.
  Exit: alter the reference rocket without touching JSON.
- [ ] **Day 6 — flight mode**
  Animated launch playback, linked telemetry plots, event timeline, summary cards + warnings.
  Exit: full launch story works offline, one click.
- [ ] **Day 7 — Flight Review + target solver**
  IREC 10K profile, constraint panel, motor/ballast search, before/after diff.
  Exit: Ascent repairs one deliberately infeasible configuration.
- [ ] **Day 8 — provenance, tests, polish** (+copilot stretch if 1–7 done)
  Evidence drawer, input hash, model/version display; fixture regression + e2e happy path; failure-state UI, demo reset.
  Exit: clean restart reproduces the same result without network.
- [ ] **Day 9 — demo hardening**
  Backup video, 90-second script rehearsed, dependency/data freeze, demo-blocking fixes only.
  Exit: demo runs cold, twice, identically.

## Non-code (parallel track)

- [x] **OpenRocket golden fixture** *(DONE 2026-07-17 — Roshan exported; lives at `data/reference/openrocket-alpha3-c6.csv`)*
- [ ] **Startup School 30-sec intro finalized** (due Jul 23) — skeleton exists; slot in one Ascent sentence.
- [ ] **Co-founder target profile draft** (due Jul 23).
- [ ] **Log build-reps** to vault `Log/` as each day's exit gate passes (progress-visibility rule).
- [ ] **Jul 18 checkpoint**: still touching the project? (Only kill signal: untouched 48 hrs.)

## Orchestrate commands (paste per day when you want a chained agent run)

Day 1: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-1] Build ascent-domain + ascent-sim motor kernel: bundled Estes C6 curve with provenance, linear thrust interpolation with implicit (0,0) start, trapezoid total impulse, impulse-proportional mass depletion, point-mass vertical derivative + RK4; Acceptance: all failing tests green; analytic constant-thrust/no-drag case matches closed form; deterministic machine-readable summary with apogee, burnout, max velocity, timestep, input hash"`

Day 2: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-2] Add flight phases to ascent-sim: rail constraint, declared drag model, 1976 standard atmosphere, event detection with bracketing (rail exit, burnout, apogee, recovery deploy, landing), timestep convergence report at dt, dt/2, dt/4; Acceptance: event timeline and summary stable at dt/2; convergence report emitted; all tests green"`

Day 3: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer,ecc:code-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-3] Freeze Alpha III + C6-5 fixture and compare ascent-sim output against manually exported OpenRocket 24.12 values; record tolerances, model differences, known assumptions in an evidence document; Acceptance: golden-fixture regression test in CI; documented agree/differ/why table; tolerance thresholds explicit"`

Day 4: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-4] Parametric vehicle model (nose, body, fins, payload, motor, chute) with component masses, CG, Barrowman CP, time-varying stability margin, and IREC 2026 constraint checks (rail-exit >=25 m/s, stability bands, fin span >=0.8 cal) with rule-source citations; Acceptance: editing mass or fins produces correct tested stability changes; constraint pass/fail unit-tested against rule values"`

Day 5: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:typescript-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-5] Tauri + React shell with rocket viewport, part inspector, motor selector; design revision tracking with dirty/stale/running/current run states; coarse IPC commands only; Acceptance: user can alter the reference rocket entirely through UI; run-state transitions correct; headless sim summary unchanged"`

Day 6: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:typescript-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-6] Flight mode: animated launch playback from completed result, linked telemetry plots, event timeline, summary cards and warnings; Acceptance: full launch story works offline with one click; playback driven only by the stored run record"`

Day 7: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-7] IREC 10K Flight Review: constraint panel bound to rule pack, deterministic motor enumeration + ballast bisection solver targeting apogee within constraints, before/after diff; Acceptance: solver repairs one deliberately infeasible configuration; result deterministic across runs; diff shows every changed parameter"`

Day 8: `/ecc:orchestrate custom "ecc:tdd-guide,ecc:rust-reviewer,ecc:code-reviewer" "[Plan: docs/FULL_BUILD_RESEARCH.md#step-8] Provenance and hardening: evidence drawer (input hash, model versions, assumptions, convergence status), fixture regression suite, end-to-end happy path, failure-state UI, demo reset; Acceptance: clean restart reproduces identical summary without network; regression suite green"`

Day 9: manual (video, rehearsal, freeze) — no agent chain; `ecc:code-reviewer` only for demo-blocking fixes.

## Rules (frozen)

- v0.1 scope frozen; no pro-tool integration before Jul 23.
- No Tauri until the headless summary is deterministic (gates Day 5 on Days 1–2).
- Every physics claim verifiable against a reference.
