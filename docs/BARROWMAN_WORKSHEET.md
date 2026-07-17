# Barrowman worksheet for Ascent v0.1

This worksheet fixes the zero-angle, subsonic static-stability model used by
Ascent v0.1.  Its aerodynamic source is Sampo Niskanen, *OpenRocket technical
documentation*, OpenRocket 13.05, 10 May 2013 (the
[techdoc](https://openrocket.sourceforge.net/techdoc.pdf)).  Equation numbers
and section names below refer to that document.

The worked vehicle is OpenRocket's bundled **Estes Alpha III / Code
Verification Rocket**.  Current OpenRocket distributions construct this
fixture in `TestRockets.makeEstesAlphaIII()` rather than shipping a literal
`alpha3.ork`; the authoritative geometry is frozen in the
[constructor](https://github.com/openrocket/openrocket/blob/3115762a8db467a1c92aa505c53cea2ed32223b8/core/src/main/java/info/openrocket/core/util/TestRockets.java#L436-L592)
and checked independently by OpenRocket's
[aerodynamic tests](https://github.com/openrocket/openrocket/blob/3115762a8db467a1c92aa505c53cea2ed32223b8/core/src/test/java/info/openrocket/core/aerodynamics/BarrowmanCalculatorTest.java#L85-L125).
That distinction matters: the similarly named bundled “A simple model rocket”
is not an Alpha III.

## Conventions and validity

- The datum is the nose tip.  `x` increases aft along the centerline; every CP
  below is converted to this global datum.  This is OpenRocket techdoc
  §3.1.4, “Coordinate systems.”
- `A_ref` is one common reference area for every component.  Here it is the
  maximum-body frontal area, `pi R^2`.  Normal-force coefficients are
  dimensionless and `CN_alpha` is per radian because angle of attack is in
  radians (techdoc §3.1.1, eqs. 3.1–3.6).
- The derivation assumes angle of attack very close to zero, steady
  non-rotational flow, a rigid rocket, a sharp nose tip, flat-plate fins, and
  an axially symmetric body (techdoc §3.2, p. 21).
- The fin equation here is the subsonic expression with
  `beta = sqrt(1-M^2)`.  The fixture is evaluated at `M = 0.30`; it must not be
  continued through `M = 1`.  OpenRocket starts moving fin CP aft above about
  `M = 0.5` and uses separate transonic/supersonic methods (techdoc §3.2.2,
  eqs. 3.35–3.49).  Therefore this fixture is a low-subsonic, small-AoA test,
  not a transonic flight model.

## Equations implemented by v0.1

### Axisymmetric body component

For a component of length `l`, local cross-sectional area `A(x)`, and volume
`V = integral_0^l A(x) dx`, OpenRocket gives

```text
CN_alpha,B = (2 / A_ref) [A(l) - A(0)]                 (alpha -> 0)
X_B,local  = [l A(l) - V] / [A(l) - A(0)]
X_B,global = x_front + X_B,local
```

Sources: techdoc §3.2.1, eq. 3.19 (the full finite-angle expression includes
`sin(alpha)/alpha`) and eq. 3.28.  `CN_alpha,B` is rad^-1; `A` and `A_ref` are
area in any one consistent squared-length unit; `V` is the corresponding
cubed-length unit; `l`, `x_front`, and `X_B` are lengths.

For a sharp nose, `A(0)=0`.  If its base area equals `A_ref`,

```text
CN_alpha,nose = 2
X_nose        = L_n - V_n / A_ref
```

This is a direct specialization of techdoc §3.2.1, eqs. 3.19 and 3.28.  A
conical nose has `V_n=A_ref L_n/3`, hence `X_nose=2L_n/3`; for any other nose
profile the volume must actually be evaluated.

A constant-diameter body tube has `A(l)=A(0)`, so its isolated classical
Barrowman derivative is zero.  Its CP is consequently undefined and it adds no
term to the CP moment sum (techdoc §3.2.1, eq. 3.19).

### Conical transition

Let a conical frustum run from fore radius `R_1` to aft radius `R_2` over
length `L`, beginning at global `x_0`.  Substituting
`V=(pi L/3)(R_1^2+R_1 R_2+R_2^2)` into the preceding body equations gives

```text
CN_alpha,tr = 2 pi (R_2^2 - R_1^2) / A_ref
X_tr,local  = L (2 R_2 + R_1) / [3 (R_2 + R_1)]
X_tr,global = x_0 + X_tr,local
```

Sources: techdoc §3.2.1, eqs. 3.19 and 3.28; conical transition geometry is
defined in Appendix A.7.  Radii and positions are lengths, `A_ref` is area,
and `CN_alpha,tr` is rad^-1.  A boattail (`R_2<R_1`) correctly produces a
negative derivative.  If `R_1=R_2`, the derivative is zero and the CP does not
enter the total.  The Alpha III fixture has no transition; this equation is
included because transitions are in the Ascent v0.1 component subset.

### Trapezoidal fin set

For one trapezoidal fin, define root chord `C_r`, tip chord `C_t`, root-to-tip
span `s`, tip-leading-edge sweep distance `X_t`, one-fin planform area
`A_fin`, and midchord sweep angle `Gamma_c`:

```text
A_fin       = s (C_r + C_t) / 2
tan Gamma_c = [X_t + (C_t - C_r)/2] / s
beta        = sqrt(1-M^2)
```

`A_fin` and `Gamma_c` are the symbols used in techdoc §3.2.2, eq. 3.40; the
area and midchord geometry are definitions supporting that equation.  Chords,
span, and sweep are lengths, area is length squared, angles are radians, and
`M` and `beta` are dimensionless.

The one-fin subsonic normal-force derivative is

```text
                       2 pi (s^2 / A_ref)
(CN_alpha)_1 = ------------------------------------------------
                1 + sqrt[1 + {beta s^2/(A_fin cos Gamma_c)}^2]
```

Source: techdoc §3.2.2, eq. 3.40.  The result is rad^-1.  It assumes a thin,
flat fin and subsonic flow.

For `N >= 3` equally spaced fins, including OpenRocket's fin-fin multiplier
`I(N_total)` and the Barrowman body-interference factor,

```text
(CN_alpha)_N      = (N/2) (CN_alpha)_1 I(N_total)
K_T(B)            = 1 + r_t/(s+r_t)
(CN_alpha)_finset = K_T(B) (CN_alpha)_N
```

Sources: techdoc §3.2.2, eqs. 3.53–3.56.  `r_t` is body radius at the fin root
(length); all three factors and `I` are dimensionless, so the result remains
rad^-1.  `I=1.000` for three or four parallel fins; the techdoc tabulates
reduced values for five or more.  The formula is not valid for arbitrary
overlapping or non-coplanar fin systems without revisiting that interference
model.

The low-subsonic trapezoidal-fin CP, measured aft from the root leading edge,
is

```text
X_f,local = (X_t/3) (C_r+2C_t)/(C_r+C_t)
          + (1/6) (C_r^2+C_t^2+C_r C_t)/(C_r+C_t)
X_f,global = x_fin,LE + X_f,local
```

Source: techdoc §3.2.2, eqs. 3.33–3.34.  Every term has length units.  The
quarter-MAC result is the low-subsonic CP; use the techdoc's Mach-dependent
extension rather than this equation above its stated range.

### Combined CP

With every component position expressed from the same datum,

```text
CN_alpha,total = sum_i (CN_alpha)_i
X_CP           = sum_i [X_i (CN_alpha)_i] / CN_alpha,total
```

Source: techdoc §3.2.1, eq. 3.29.  Signed derivatives must be retained, notably
for boattails.  `CN_alpha,total` is rad^-1 and `X_CP` is length; CP is undefined
if the denominator is zero.

## Worked Alpha III example

### Source geometry and reference condition

OpenRocket's frozen constructor supplies:

| Quantity | Symbol | Value |
|---|---:|---:|
| Tangent-ogive nose length | `L_n` | 0.070 m |
| Body radius / diameter | `R`, `D` | 0.012 m / 0.024 m |
| Body-tube length | `L_b` | 0.200 m |
| Fin count | `N` | 3 |
| Root chord | `C_r` | 0.050 m |
| Tip chord | `C_t` | 0.030 m |
| Span | `s` | 0.050 m |
| Tip-leading-edge sweep | `X_t` | 0.020 m |

The fins are bottom-aligned on the body, so their root leading edge is

```text
x_fin,LE = L_n + L_b - C_r = 0.070 + 0.200 - 0.050 = 0.220 m.
```

This placement arithmetic follows the global datum of techdoc §3.1.4.  The
evaluation point is `M=0.30`, the default low-subsonic condition used by
OpenRocket's hand-checked Alpha III aerodynamic test.

### Reference area and nose

```text
A_ref = pi R^2
      = pi (0.012)^2
      = 0.0004523893421 m^2.
```

This reference-area choice follows techdoc §3.1.1.  The nose is the
constructor's default-parameter tangent ogive.  From Appendix A.2, eqs.
A.2–A.5, its generating-circle radius and offset are

```text
rho = (L_n^2 + R^2)/(2R)
    = (0.070^2 + 0.012^2)/(2 x 0.012)
    = 0.2101666667 m

a = rho - R = 0.1981666667 m
r(x) = sqrt[rho^2-(L_n-x)^2] - a.
```

Integrating that profile exactly (techdoc Appendix A.2 geometry, then §3.2.1
eq. 3.28):

```text
V_n/pi = (rho^2+a^2)L_n - L_n^3/3
       - a[L_n a + rho^2 asin(L_n/rho)]
       = 5.4209208298e-6 m^3

V_n = 0.00001703032505 m^3
V_n/A_ref = 0.03764528354 m

CN_alpha,nose = 2 A_ref/A_ref = 2.000000 rad^-1
X_nose = 0.070 - 0.03764528354
       = 0.03235471646 m.
```

OpenRocket's test rounds the same nose CP to `0.03235 m`.

### Body tube

```text
CN_alpha,body = (2/A_ref)(A_ref-A_ref) = 0 rad^-1.
```

By techdoc §3.2.1, eq. 3.19, the constant-diameter tube contributes no
classical zero-AoA CP moment.  Its component CP is recorded as `null` in the
fixture rather than inventing a location for a zero-force component.

### Fins

First calculate the one-fin geometry and compressibility terms:

```text
A_fin = 0.050(0.050+0.030)/2 = 0.0020000000 m^2

tan Gamma_c = [0.020+(0.030-0.050)/2]/0.050
            = 0.010/0.050 = 0.2
Gamma_c = 0.1973955598 rad
cos Gamma_c = 0.9805806757

beta = sqrt(1-0.30^2) = 0.9539392014
beta s^2/(A_fin cos Gamma_c)
  = 0.9539392014(0.050^2)/(0.0020000000 x 0.9805806757)
  = 1.2160386507.
```

Apply techdoc §3.2.2, eq. 3.40:

```text
(CN_alpha)_1
  = [2 pi(0.050^2/0.0004523893421)]
    / [1+sqrt(1+1.2160386507^2)]
  = 13.4874765047 rad^-1.
```

For three equally spaced fins, `I(3)=1`, and techdoc §3.2.2, eqs. 3.53–3.56
give

```text
(CN_alpha)_3 = (3/2)(13.4874765047)(1)
             = 20.2312147571 rad^-1

K_T(B) = 1 + 0.012/(0.050+0.012)
       = 1.1935483871

CN_alpha,finset = 1.1935483871 x 20.2312147571
                = 24.1469337423 rad^-1.
```

Apply techdoc §3.2.2, eq. 3.34 for position:

```text
X_f,local
  = (0.020/3)(0.050+2x0.030)/(0.050+0.030)
    +(1/6)(0.050^2+0.030^2+0.050x0.030)/(0.050+0.030)
  = 0.0091666667 + 0.0102083333
  = 0.0193750000 m

X_f,global = 0.220 + 0.019375 = 0.2393750000 m.
```

OpenRocket's implementation-level test reports `0.0193484 m` because its
generic fin calculator obtains MAC geometry by discretizing the planform.  The
equation-exact value above differs by only `0.0000266 m` (0.027 mm); Ascent's
analytic fixture intentionally tests eq. 3.34, not that implementation
quadrature.

### Total CP

Using techdoc §3.2.1, eq. 3.29:

```text
CN_alpha,total = 2.0000000000 + 0 + 24.1469337423
               = 26.1469337423 rad^-1

normal-force moment
  = 2.0000000000(0.03235471646)
    +24.1469337423(0.2393750000)
  = 0.06470943292 + 5.78017226457
  = 5.84488169749 m/rad

X_CP = 5.84488169749 / 26.1469337423
     = 0.22353985194 m from the nose tip.
```

As a cross-check, substituting OpenRocket's discretized fin CP and rounded
nose CP yields its frozen test value `0.2235154 m`.

### Expected loaded-C6 CG

OpenRocket's
[mass-calculator test](https://github.com/openrocket/openrocket/blob/3115762a8db467a1c92aa505c53cea2ed32223b8/core/src/test/java/info/openrocket/core/masscalc/MassCalculatorTest.java#L134-L210)
freezes the code-verification rocket's dry mass and axial CG as
`m_dry=0.0252682918461 kg` and `x_dry=0.191768435800 m`.  The motor mount begins
at `0.070+0.133=0.203 m`; a 70 mm C6 is therefore centered at
`x_C6=0.203+0.070/2=0.238 m`.  For consistency with the flight kernel, loaded
motor mass is `0.0241 kg` from Ascent's NAR-certified
[`estes_c6.json`](../crates/ascent-domain/data/motors/estes_c6.json).

The ordinary first-moment mass balance (not a Barrowman aerodynamic equation)
is

```text
x_CG,loaded = (m_dry x_dry + m_C6 x_C6)/(m_dry+m_C6)

dry moment   = 0.0252682918461 x 0.191768435800
             = 0.004845660803 kg m
motor moment = 0.0241 x 0.238
             = 0.005735800000 kg m
loaded mass  = 0.0252682918461 + 0.0241
             = 0.0493682918461 kg

x_CG,loaded  = (0.004845660803+0.005735800000)/0.0493682918461
             = 0.214337187028 m from the nose tip.
```

At ignition, the equation-exact static margin is consequently

```text
(X_CP-X_CG)/D
  = (0.223539851942-0.214337187028)/0.024
  = 0.3834443714 calibers.
```

This CG is specific to the official OpenRocket code-verification structure
combined with Ascent's certified C6 mass.  It is not a claim that every retail
Alpha III build has the same mass distribution; glue, finish, recovery packing,
and motor-retention choices move the measured CG.

## Machine-readable values

The unrounded inputs, component results, totals, provenance, and tolerances are
in [`data/fixtures/alpha3_stability.json`](../data/fixtures/alpha3_stability.json).
Displayed arithmetic is rounded only after the machine-readable values are
computed.
