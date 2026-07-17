# RASP `.eng` format

Source: [ThrustCurve, “RASP File Format”](https://www.thrustcurve.org/info/raspformat.html).
RASP/ENG is a line-oriented thrust-curve interchange format.

## Entries and comments

An entry may begin with blank lines or comment lines. A comment line begins
with `;`; comments and blanks are ignored before the header. After the final
sample, an entry may end or be followed only by comments/blanks. A file can
contain multiple motors: separate adjacent entries with at least one comment
line (a single `;` is conventional) so a parser does not run them together.

## Header

The first non-comment/nonblank line has exactly seven whitespace-separated
fields:

```text
common_name diameter_mm length_mm delays propellant_kg loaded_mass_kg manufacturer
```

| Field | Meaning and unit |
|---|---|
| `common_name` | Common motor designation, normally impulse class plus average thrust (for example `C6`). |
| `diameter_mm` | Casing diameter in millimetres. |
| `length_mm` | Casing length in millimetres. |
| `delays` | Hyphen-separated available delays; `0` means ejection charge with no delay; `P` means plugged/no ejection charge. |
| `propellant_kg` | Consumable mass in kilograms: propellant for a solid motor; fuel plus oxidizer for a hybrid. |
| `loaded_mass_kg` | Ready-to-fly motor mass in kilograms. |
| `manufacturer` | Short manufacturer abbreviation. |

Real files vary in spacing, decimal precision, delay spelling, and the final
manufacturer token (`E`, `Estes`, or `AT` are all encountered). Preserve the
raw header while normalizing values separately.

## Data pairs and termination

Each following data line is `time_s thrust_n`, two floating-point numbers.
`time_s` must be strictly increasing. RASP assumes an implicit `(0, 0)` point;
do not add an explicit initial zero pair. The last sample must be zero thrust;
its time is the burn time. An early zero terminates the entry, so any following
points are invalid. Classic RASP allowed 32 samples including that terminal
zero; current tools may accept more, but compatibility importers should warn.

The conformance corpus in `data/eng-samples/` intentionally includes normal
examples, a multi-entry file, header variations, and a missing-terminal-zero
negative fixture. Import behavior should preserve raw content, parse valid
entries, and diagnose malformed termination rather than silently inventing a
burnout sample.
