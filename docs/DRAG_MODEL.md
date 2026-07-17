# Drag model for an Alpha III-class v0.1 rocket

For the v0.1 low-power flight model, use `D = 0.5 rho V_air^2 Cd A_ref` with
air-relative speed and frontal reference area.  The recommended artifact is the
narrow, source-tagged table in [`data/aero/alpha3_cd.json`](../data/aero/alpha3_cd.json),
not a claim that Ascent has a general geometry-to-drag solver.

## What a constant Cd misses

`Cd` changes with Reynolds number, Mach number, angle of attack, surface finish,
and component joints.  It also changes through the flight: a stationary rocket
has different skin-friction behavior than a fast coast, and base drag depends on
the wake.  A flat value is useful as an explicitly declared approximation, but
it cannot distinguish a smooth painted tube from a rough tube, a lug from no
lug, or low-speed flow from transonic flow.

OpenRocket's low-speed drag treatment separates the relevant contributors:

- nose and body skin friction;
- body pressure/form drag;
- base drag;
- fin skin friction and fin pressure drag; and
- launch-lug drag.

That is the standard subsonic build-up for this class of rocket.  It is useful
for explaining sensitivity, but its component correlations are not a substitute
for calibration.  Source: Sampo Niskanen, *OpenRocket technical documentation*,
§3.3 “Aerodynamic drag,” <https://openrocket.sourceforge.net/techdoc.pdf>.

## Published/reference values used here

The only numeric Cd values recommended for this repository’s first demo table
come from the checked-in OpenRocket 24.12 export of a 25 mm, C6-powered bundled
model rocket.  The export records `Cd=0.875` at the static row, `0.618` at
Mach 0.100, `0.623` at Mach 0.200, and `0.632` at its highest ascending Mach
sample (0.281).  The source is
[`data/reference/openrocket-alpha3-c6.csv`](../data/reference/openrocket-alpha3-c6.csv);
the exact row provenance and validity label accompany every number in the JSON.
It is an Alpha III-*class* reference, not a measurement of a retail Alpha III.

`docs/EVIDENCE.md` independently summarizes the same export as about `0.87`
static and `0.63` in coast.  The recommended `.3`, `.4`, and `.5` breakpoints
hold the source’s last observed `0.632` value.  Those are deliberately labeled
as an unvalidated policy extrapolation, not published aerodynamic data.

## Recommendation for Day 3

Day 3 should replace the current flat `Cd=0.60` with the checked-in
Mach-indexed table for the one reference configuration, display its
Alpha-III-class/Mach-0-to-0.4 validity metadata, and retain a warning outside
that range.  `0.60` is slightly **low** relative to the source’s approximately
`0.63` coast value, so it tends to overpredict apogee; that direction matches
the current vertical result of 361 m versus Estes’ advertised roughly 335 m.
Do not tune a universal constant to erase that gap: the compared vehicle,
wind, release timing, and 1-D model differ.  Re-run the frozen OpenRocket
comparison after the change and preserve the result as evidence.
