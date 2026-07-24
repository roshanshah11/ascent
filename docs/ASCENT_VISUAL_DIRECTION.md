# Ascent visual direction

## Purpose

Ascent is an engineering review environment for understanding a real vehicle
and a real flight trace. The scene should feel like an instrumented field
review, not a game HUD, a generic SaaS dashboard, or a prerecorded launch
movie.

## One-sentence scene

An engineer reviews a Black Brant IX flight at a White Sands launch site in
late-afternoon desert light, using a quiet, trace-backed instrument surface
that leaves the vehicle and its motion as the primary visual subject.

## Visual hierarchy

1. Vehicle, plume, terrain, and flight motion occupy the frame.
2. Mission state and playback time are immediately legible.
3. Engineering layers and evidence are available at the edges, never as a
   full-screen obstruction.
4. Detailed evidence appears only after the trace is ready or when requested.

At 1920x1080, the live viewport must retain at least 70% of the visible area.

## UI language

- Use a restrained dark graphite overlay with sufficient contrast, never black
  text on dark glass and never default Unity white buttons.
- Use cyan for accepted trace truth and current selection. Use amber only for
  caveats, missing data, and warnings. Reserve red for failure.
- Use compact edge panels with full borders or subtle background separation.
  Avoid opaque full-width controls and dashboard-card grids.
- Use a clear typographic hierarchy: mission name, run state, then supporting
  data. Every displayed numeric value has a unit and provenance label.
- Keep controls short and direct: Play, Pause, Ready, Loading trace, Trace
  unavailable. Do not display a keyboard shortcut unless it is implemented.
- Prefer icon plus short label for camera/layer controls. Ensure focus and
  keyboard navigation remain visible.

## Environment and camera

- White Sands is warm gypsum, pale sand, low dunes, strong atmospheric depth,
  and physically credible sun direction.
- The launch vehicle needs readable silhouette separation from sky and terrain.
- Use pad camera for context, chase for motion, onboard for attitude, ground
  tracking for geography, and inspection for close engineering review.
- Camera changes never alter simulation time.
- Plume, stage separation, position, and orientation come only from the
  accepted Rust trace.

## Explicitly avoid

- Default Unity button, toggle, and panel styling.
- Full-screen opaque UI masking the scene.
- Dark-on-dark text, low-contrast grey labels, or unreadable controls.
- Neon sci-fi, arcade telemetry, faux cockpit chrome, and generic SaaS cards.
- Decorative data with no source, unit, or validity state.
- Calling a static scene, an empty UI, or a frame-rate probe a completed review.

## Runtime acceptance

Before a build is called visually ready, prove all of the following in the
packaged player:

1. It loads the authoritative bridge trace and visibly reaches `Ready`.
2. The trace hash is visible and the vehicle/plume respond to playback.
3. Space, camera selection, clean view, and visible layer controls work.
4. The bridge shuts down with no orphan child process.
5. A 1920x1080 screenshot is captured after `Ready` and reviewed against this
   document. Reject the build if default styling, weak contrast, or UI
   obstruction remains.

## Agent workflow

Use `art-direction` before changing the scene's visual treatment. Use
`unity-ui-toolkit-design-system` selectively for UXML and USS components, but
do not adopt its generic game theme. Use `unity-developer` for HDRP, runtime,
packaging, and target-machine verification. A runtime screenshot review is a
required gate, not optional polish.
