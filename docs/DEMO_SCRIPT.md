# 90-second Ascent demo

This is a Day 9 rehearsal script for the Day 1–8 vertical slice described in
`docs/PROJECT_SPEC.md` and `docs/FULL_BUILD_RESEARCH.md` §“The 90-second July
23 demo.”  It claims only planned, deterministic surfaces: parametric vehicle
editing and CP/CG (Day 4), flight plots (Day 6), IREC review and feasible
search (Day 7), and provenance/evidence (Day 8).

| Time | Screen | Roshan says |
|---:|---|---|
| 0–8 s | Open **IREC 10K / single-stage COTS**. The rocket, CP, CG, target apogee, rail speed, and stability appear. | “This is Ascent: a design and flight-review workspace for one competition rocket.” |
| 8–19 s | Select the supplied rule-violating design. Constraint panel shows red stability and rail-speed rows; the geometry remains visible. | “This design is not flyable under the rule set. The panel names the failed constraints instead of hiding them in a warning.” |
| 19–31 s | Open the violated-rule detail. Show the computed value, threshold, and cited rule ID. | “Each result is computed from the design and points back to the governing rule.” |
| 31–45 s | Click **Find feasible configuration**. Show the before/after diff: selected motor, ballast, predicted apogee, rail speed, and stability all update. | “I can ask for a feasible configuration. The search tries the bundled motors and ballast values, then returns the change it made.” |
| 45–56 s | Constraint panel turns green. Keep the before/after diff visible. | “Now the same review is green: it reaches the target while staying inside the rail-speed and stability limits.” |
| 56–70 s | Click **Launch**. Flight view animates altitude, velocity, and acceleration; timeline marks rail exit, burnout, apogee, deployment, landing. | “Launch runs the declared point-mass model. The timeline shows the flight events, not just one apogee number.” |
| 70–83 s | Open **Evidence**. Show input hash, model version, drag assumption, convergence status, and frozen OpenRocket comparison. | “Here is the evidence behind that number: the exact input hash, model assumptions, convergence check, and a frozen reference comparison.” |
| 83–89 s | Return to the repaired design and green review. | “That gives a team a reproducible answer to one question: is this design feasible, and what evidence supports it?” |

Total: **89 seconds**.

## Backup-video shot list

| Shot | Duration | What it proves |
|---|---:|---|
| IREC 10K design opens with CP/CG and review strip | 6 s | The app owns a concrete vehicle and exposes computed review inputs. |
| Red constraint panel on the violating design | 8 s | Constraint checking produces a visible pass/fail decision. |
| Rule-detail drawer with value, bound, and rule ID | 7 s | The decision has traceable rule provenance. |
| Feasible-search click and before/after diff | 13 s | Repair is an explicit deterministic design action, not a text suggestion. |
| Green review after repair | 6 s | The repaired configuration satisfies the shown checks. |
| Launch animation plus event timeline | 14 s | Simulation produces time-series and named flight events. |
| Evidence drawer | 12 s | Results preserve input identity, model assumptions, convergence, and reference evidence. |
| Return to repaired vehicle | 5 s | The narrative ends on the actionable design state. |

Total backup video: **71 seconds**.  Leave 19 seconds of live-demo margin for
cursor movement and panel transitions.

## Live-failure fallback narration

“The planned flow opens a failing IREC design, shows the exact failed rule, and
uses a deterministic search to produce a feasible version. The launch view
turns that repaired design into a flight timeline. The evidence drawer records
the inputs, model assumptions, convergence result, and frozen reference used to
judge the number.”
