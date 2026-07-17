# ASCENT — AI-Native Rocket Engineering Studio

*Locked 2026-07-16. v0.1 demo due 2026-07-23 for YC Startup School (Jul 25–26 SF).*

## One-liner

Design a rocket, simulate the flight with honest physics, and iterate with an AI copilot that understands aerodynamics — the modern successor to OpenRocket, growing into the unified front end for the free professional aerospace stack.

## The thesis (the Startup School answer)

There is a canyon in aerospace tooling. On one side: OpenRocket, a 20-year-old Java app — what every hobbyist and collegiate rocketry team designs on. On the other: the professional stack (CFD, Simulink/HIL, STK), $100K+/seat, built for certification. In between: **nothing** — even though the free tier of the pro stack actually exists (OpenFOAM, OpenVSP, GMAT are all free and production-grade) and nobody can use it, because it's six disconnected tools with brutal learning curves. Ascent starts as the OpenRocket replacement and grows into the layer that makes the free professional stack usable by a college rocketry team. "Figma moment for aerospace design tools."

Market-gap evidence: `~/Downloads/OpenRocket vs. the Professional Aerospace Stack.md` (Roshan's own research, 2026-07-16).

## v0.1 — the 9-day slice (~25 hrs, Mac-only, $0)

*Revised 2026-07-16 after Codex's `FULL_BUILD_RESEARCH.md` (24 primary sources). The wedge is now the IREC competition flight-review job.*

An app where a competition team can:
1. **Design** a single-stage rocket from parts (nose, body, fins, payload, motor, chute) — live preview, CP/CG markers, live stability vs IREC DTEG constraint bands (min 7.5% subsonic / rail-exit ≥ 25 m/s / fin span ≥ 0.8 cal).
2. **Pick a motor** from three bundled, provenance-preserved thrust curves (live ThrustCurve search post-demo; API verified working 2026-07-16).
3. **Launch** — honest, explicitly-scoped 2D point-mass sim (not fake "6-DOF"): RK4, thrust-curve burn, mass depletion, declared drag model, rail constraint, chute recovery, event timeline. Live plots.
4. **Find feasible configuration** — deterministic motor/ballast search that repairs a rule-violating design to hit target apogee within constraints, returning a before/after diff. The "smart" feature, no LLM required.
5. **Evidence drawer** — input hash, model versions, assumptions, convergence status, comparison vs a frozen OpenRocket reference run.
6. **Copilot (stretch, days 8–9 only if 1–5 are done):** thin LLM loop over the already-typed actions (replace_motor, add_ballast, run_simulation, check_constraints). Demo ends "now ask it in English" if it works; stands alone if not.

**Anti-slop artifact:** layered validation — analytic invariants, timestep convergence, and a golden fixture vs OpenRocket's output for the reference rocket (Alpha III on C6-5). Every run labeled Verified / Consistent / Warning / Invalid.

**Stack:** Rust physics core (workspace: domain ← aero ← sim ← optimize, per Codex architecture) + Tauri/React UI. Decided 2026-07-16.

## Build order (risk-first)

Follow the day-by-day plan in `FULL_BUILD_RESEARCH.md` §Build sequence — it's better than what was here. Summary: Day 1 motor kernel + tests → Day 2 phases/convergence → Day 3 OpenRocket reference fixture → Day 4 parametric vehicle + Barrowman stability → Day 5 Tauri shell + design mode → Day 6 flight mode → Day 7 IREC review + target solver → Day 8 provenance/tests (+copilot stretch) → Day 9 demo hardening + backup video. No Tauri until the headless summary is deterministic.

## Roadmap (post-SS — where Roshan's pro-tool access slots in)

- **v0.2 — validation ladder:** import/compare against OpenRocket + RASAero II ("we validate against everything").
- **v0.3 — real geometry:** OpenVSP under the hood for parametric flight geometry + vortex-lattice aero (the bridge fidelity tier).
- **v0.4 — CFD escalation:** one-click "check this design in OpenFOAM" — Ascent writes the case files, runs the solve, renders the result. This is the moat: nobody can make OpenFOAM usable; Ascent makes it a button.
- **v0.5 — trajectory:** GMAT integration for the teams flying high-altitude/guided projects.
- MATLAB/Simulink/STK student access = Roshan's personal learning ladder for the GNC layer, not product dependencies.

## Rules

- v0.1 scope is frozen. No pro-tool integration before Jul 23 — they are roadmap, not demo.
- Every physics claim must be verifiable against a reference (OpenRocket, published flight data).
- Locked until Jul 18 checkpoint: the only kill signal is "didn't touch it for 48 hours," not "doesn't spark."

## Tonight (first task, ~1 hr)

`git init` → Rust crate → simulate Estes Alpha III on C6-5 → print apogee → compare to OpenRocket (~1,100 ft). One number, verified, tonight.
