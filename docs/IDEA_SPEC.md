# ASCENT — Idea Spec (the full thing, no version slicing)

*Drafted 2026-07-16 from Roshan's convergence sessions + market research. Companion: `PROJECT_SPEC.md` (the 9-day execution slice). This doc is the idea itself.*

## The problem, in one paragraph

Everyone who designs and flies rockets below the professional tier — hobbyists, high-power certification flyers, and hundreds of university competition teams — runs on a fragmented, aging toolchain. The standard design tool is OpenRocket, a 20-year-old Java app. The standard tools *disagree with each other*: the same design predicts [11,453 ft in OpenRocket and 9,224 ft in RASAero II](https://www.rocketryforum.com/tags/openrocket/), and validation studies show RockSim/RASAero underpredict apogee while OpenRocket overpredicts. Teams doing serious work hit the ceiling immediately: no programmatic access ([IREC 2025 teams wrote MATLAB–Java bridges just to script OpenRocket](https://www.soundingrocket.org/uploads/9/0/6/4/9064598/irec2025_podium_abstracts__1_.pdf)), no version control ([.ork files are zipped XML, git-hostile](https://uwaterloo.atlassian.net/wiki/spaces/ROCKETRY/pages/43242520609/Flight+Simulation+Software+Exploration+Design+Doc)), no path to higher fidelity (CFD runs took teams [hours per case, 34 cases for one aero database](https://www.sciencedirect.com/science/article/pii/S1270963826001392)). The demand evidence is that top teams keep *building their own simulators from scratch* — Sydney's Ironbark (won SAC 2022), Cowboy Rocketworks' MATLAB port of OpenRocket's own methodology, Waterloo's formal tool-comparison project. When your users are writing their own replacement for your product, the market is screaming.

## The idea

**Ascent is the engineering environment for sub-professional rocketry: design, simulate, validate, and iterate on a rocket in one modern native app, with an AI copilot that works against real physics.**

*Initial application (locked 2026-07-16, per Codex research):* **design a competition rocket to hit its target apogee, pass IREC flight-review constraints, and produce a reproducible evidence package.** The AI-native environment is the company; the flight-review job is the wedge — a complete, primary-source-documented job (IREC 2026 rules require predicted apogee, rail-departure velocity ≥ 25 m/s, stability bands, sim file, and graphs; scoring rewards apogee accuracy). See `FULL_BUILD_RESEARCH.md`.

Not a prettier OpenRocket — an opinionated rebuild of the whole workflow:

1. **Design** — parametric rocket builder (parts, materials, motors) with live 3D preview and live stability analysis. Real motor data (ThrustCurve.org, every certified motor).
2. **Simulate** — honest 6-DOF flight simulation (thrust, mass depletion, aero, wind, recovery), with results a team can defend: every number traceable to a method, every method validated against reference tools and real flight data.
3. **Cross-validate** — the killer feature no tool has: run the same design through multiple engines (native core, OpenRocket, RASAero II, RocketPy) and show the spread. Today teams do this by hand across four apps and argue about which number to trust. Ascent makes disagreement-between-models a first-class UI object.
4. **Copilot** — an AI that designs *against the physics*: "make this stable with a 200g payload," "get me under the SAC 10k ft ceiling with margin" → proposed changes → re-simulated → before/after diff. The AI never asserts; it proposes and the simulator verifies. (This is the anti-slop architecture: generation gated by verification.)
5. **Escalate fidelity** — *revised per Codex research 2026-07-16:* the honest ladder is (a) imported aero coefficient tables (Mach/AoA, with source + validity metadata) from RASAero, wind-tunnel, CFD, or team data — the safest fidelity boundary; (b) OpenVSP as geometry interop only until a rocket-specific validation proves VSPAERO adds fidelity; (c) OpenFOAM as "export validated CFD case" first, never "one-click accurate CFD"; (d) GMAT killed — orbital mission design, wrong job. The free professional stack exists; Ascent is its missing interface — but each bridge ships only with a validation case behind it.
6. **Team-native** — projects as plain-text files that diff and merge (git-friendly, unlike .ork), simulation runs as reproducible records, dispersion/Monte Carlo analysis for range-safety submissions.

## Who it's for (concentric rings)

- **Core:** university competition teams (Spaceport America Cup ~150 teams/yr, IREC, UKROC, EuRoC) — they have real deadlines, real money, documented pain, and they already build their own tools. UChicago has a team; Roshan lands there in September.
- **Second ring:** high-power hobbyists (L1–L3 certification flyers) — the ThrustCurve/RocketryForum population.
- **Outer ring:** the education market (intro aero courses, high-school TARC teams) and eventually early-stage new-space startups doing sounding-rocket-class work before they can afford the pro stack.

## Competitive landscape (as of Jul 2026)

- **OpenRocket** — free, standard, still developed (latest stable 24.12, Jul 2025; 26.xx on unstable branch — do NOT pitch it as abandoned), 20-year-old Java UI, no AI, no documented batch CLI, git-hostile format. The incumbent to replace.
- **RockSim (~$130), SpaceCAD (~$60)** — commercial, dated, no AI, no ladder.
- **RASAero II** — free, best transonic/supersonic accuracy, famously unfriendly UI. A validation target, not a competitor.
- **RocketPy** — MIT-licensed Python library, peer-reviewed, [validated to ~1% apogee error against real university flights](https://github.com/RocketPy-Team/RocketPy). A library, not a product — no UI, no design mode. **Potential ally/component, and proof the physics tier is achievable.**
- **ARX (Irenova)** — [posted Feb 2026 on RocketryForum](https://www.rocketryforum.com/threads/testing-arx-an-openrocket-alternative-with-genai-and-modern-ui-feedback-wanted.196256/): "OpenRocket alternative with GenAI and modern UI." Solo founder, MVP, motor-optimization-focused, self-described "not a proper rocketry tool as of now," seeking investors. **Validates the thesis (someone else sees it) without occupying it.** Watch; don't panic.
- **Ironbark et al.** — team-internal simulators. Not products; they're the demand signal.

**The open lane:** nobody combines (a) modern native design UX, (b) verification-gated AI, (c) cross-engine validation, (d) the fidelity ladder to the free pro stack. ARX has a slice of (a)+(b) at MVP quality. The moat candidates are (c) and (d) — deep engineering work that a wrapper can't fake.

## Why Roshan, why now

- The "AI that must survive a physics check" architecture is the anti-slop thesis made into a product — his stated identity target.
- Timing: AI codegen makes a solo builder able to attack a scope (native app + physics core + tool bridges) that needed a team in 2020; meanwhile the slop backlash makes "verified engineering" the differentiator.
- Distribution: he physically joins a target-market team (UChicago rocketry) in September.
- The numerate-builder profile (arXiv paper, shipped native app, PE analyst) matches a product whose whole value is trustworthy numbers.

## Core design principles

1. **Verification gates generation.** No AI output reaches the user without passing through the simulator. The copilot proposes; physics disposes.
2. **Show the error bars.** Every prediction carries its validity regime and its spread across engines. Trustworthiness > false precision. (This is what "not a toy" means to engineers.)
3. **Plain-text truth.** Designs are human-readable files that diff, merge, and script. The .ork lesson.
4. **The ladder, not the lock-in.** Ascent wraps and orchestrates open tools (RocketPy, OpenVSP, OpenFOAM, GMAT) rather than reinventing them past the core tier. Interop imports (.ork, .eng, RASAero) are table stakes.
5. **Native and fast.** Design iteration is a flow-state activity; sims run in milliseconds locally. No cloud round-trips for the core loop.

## Open questions (Roshan's research list — the "what I need to know" map)

1. **Physics core: build vs. wrap.** Write the Barrowman/6-DOF core in Rust (max defensibility, max effort) vs. embed RocketPy (validated, MIT, Python — but then the "hard part" is someone else's)? Middle path: Rust core for the interactive loop, RocketPy as one of the cross-validation engines. → Study RocketPy's architecture and its validation paper first.
2. **Aero methods:** Barrowman extended (OpenRocket techdoc) — what exactly breaks transonic, and what does RASAero do differently? (Reading list: [OpenRocket techdoc](https://openrocket.sourceforge.net/techdoc.pdf), RASAero documentation, Box's "Estimating the dynamic and aerodynamic parameters of passively controlled high power rockets.")
3. **File format:** what does the git-friendly design format look like (TOML/JSON schema), and how faithful can .ork import be?
4. **Copilot mechanics:** tool-use loop where the LLM calls the simulator — what's the right action space (edit part params? swap motors? add mass?) and how to keep it from local-optimum thrashing?
5. **OpenFOAM bridge feasibility:** what's the minimum honest CFD case for a rocket body (axisymmetric? cut-cell?) that runs on a MacBook in minutes not hours?
6. **Community:** what did the 17 replies to ARX actually ask for? (Read the [full thread](https://www.rocketryforum.com/threads/testing-arx-an-openrocket-alternative-with-modern-ui-feedback-wanted.196256/) — it's free user research.) What do SAC technical reports complain about, workflow-wise?
7. **Business shape (later):** free for hobbyists / paid for teams? Open-core? Don't decide now; note that every incumbent under $150 signals price ceiling for individuals — teams with budgets are the revenue ring.

## Risks

- **OpenRocket ships a modern UI** — mitigated by (c)/(d): the moat was never the UI alone.
- **ARX raises and executes** — watch the thread; their focus (motor optimization) differs from Ascent's (whole-workflow + ladder).
- **Physics is subtly wrong** — the cross-validation feature converts this risk into the product's core competence: disagreement is surfaced, not hidden.
- **Scope gravity** — the ladder invites infinite integration work. Rule: a bridge ships only when a real team asks for it.
- **Roshan's history** — projects die when invisible. Mitigation: build-reps logged in the vault, 48-hr kill-signal checkpoints, SS deadline as forcing function.
