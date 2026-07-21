# Unguided Review Protocol — Criterion 7

Purpose: run the **one** remaining gate that flips `docs/UNITY_PLATFORM_DECISION.md`
from `contain` to `expand`. It is a human measurement by design — the whole point
is that a person who was *not* told what to look at can still read the mission from
the render alone. Follow this exactly; do not coach the reviewer.

**Time:** ~10 minutes. **You need:** one technically-literate person (engineer,
physics/aero background, or a capable hobbyist) who has **not** seen this build or
been briefed on the flight. That "cold" condition is the measurement — a briefed
reviewer invalidates it.

---

## 1. Build & launch (operator, before the reviewer arrives)

```bash
# from repo root, editor 6000.0.79f1 must be installed
bash scripts/unity.sh -batchmode -quit \
  -executeMethod Ascent.Editor.BlackBrantSceneBuilder.BuildFromBatch
bash scripts/unity.sh -batchmode -quit \
  -executeMethod Ascent.Editor.PlayerBuilder.BuildFromBatch
open visualizer/AscentUnity/Build/StandaloneOSX/BlackBrantIX.app
```

Get the review scene playing (ignition through apogee, camera cuts running). Then
**stop talking.** Hand over the keyboard/screen. Do not narrate, point, or answer
"what am I looking at" — the answer to that question *is* the test.

---

## 2. The only thing you say to the reviewer

> "This is a rocket flight. Watch it, then tell me what happened. Ask me nothing
> about the flight itself — I won't answer. When you're ready, I'll ask six
> questions."

Let them watch it at least once. Replays / seeking are allowed; hints are not.

---

## 3. The six questions (ask verbatim, record the answer, no follow-up coaching)

| # | Ask exactly | PASS if the reviewer, unprompted, can… |
|---|-------------|----------------------------------------|
| 1 | "When does the motor light?" | point to ignition/liftoff (first powered instant) |
| 2 | "Does the vehicle ever come apart, and when?" | identify stage separation and roughly when |
| 3 | "Where's the top of the flight?" | identify apogee (highest point / velocity sign-flip) |
| 4 | "At the end, which stage are we watching?" | name the active/upper stage vs. the spent one |
| 5 | "Which way is the vehicle pointing during coast?" | describe orientation (nose attitude) coherently |
| 6 | "Pick any number on screen — where does it come from?" | trace one displayed value to its meaning/source |

A "PASS" is the reviewer getting it **without** you confirming or steering. Close
enough counts (they don't need exact seconds); fundamentally wrong or "I can't tell"
is a FAIL for that item.

**Criterion 7 passes only if all six pass.** Partial is a FAIL — record which ones
missed, because those point at the specific readouts/visuals to improve next.

---

## 4. Record the result (operator)

Fill this in and paste it into `docs/UNITY_PLATFORM_DECISION.md` under criterion 7,
replacing the "NOT DONE (human)" row's basis:

```
Unguided review — <YYYY-MM-DD>
Reviewer background: <role, one line; confirm not previously briefed>
  1 ignition ......... PASS/FAIL  (quote/paraphrase their answer)
  2 separation ....... PASS/FAIL
  3 apogee ........... PASS/FAIL
  4 active stage ..... PASS/FAIL
  5 orientation ...... PASS/FAIL
  6 value source ..... PASS/FAIL
Verdict: <ALL SIX PASS → criterion 7 MET> / <n failed → still NOT MET>
```

---

## 5. If all six pass → flip the decision

In `docs/UNITY_PLATFORM_DECISION.md`:
- Criterion 7 row → **MET**, basis = the recorded result above.
- Header + summary → **Decision: `expand`** (all seven criteria now met).
- Keep the "scope preserved" section — Unity stays additive regardless.

If any fail, the decision stays `contain`; the failed items are the punch list.
No fabrication, no averaging across multiple reviewers to manufacture a pass — one
clean cold review, recorded honestly.
