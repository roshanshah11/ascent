using System.Collections.Generic;
using System.Globalization;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Trace;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// One State-panel line: a labelled value with its unit and evidence
    /// provenance. Every field the review shows about the vehicle is one of
    /// these, so no numeric value is ever displayed without a unit and a
    /// source grade.
    /// </summary>
    public readonly struct StateReadout
    {
        public readonly string Label;
        public readonly string Value;
        public readonly string Unit;
        public readonly EvidencePolicy Provenance;

        public StateReadout(string label, string value, string unit, EvidencePolicy provenance)
        {
            Label = label;
            Value = value;
            Unit = unit;
            Provenance = provenance;
        }
    }

    /// <summary>
    /// Derives the human-facing flight state (altitude, speed, Mach, stage,
    /// phase) from the accepted <see cref="FlightTrace"/> at a given review
    /// time. This never simulates: altitude and speed are read straight from
    /// the trace channels; stage and phase are read from the trace's own event
    /// stream; Mach is the only approximation (sea-level ISA sound speed) and is
    /// graded as such. Pure and headless-testable — no Unity scene required.
    /// </summary>
    public static class FlightReadout
    {
        /// <summary>ISA sea-level speed of sound (m/s). Mach is graded Approximate because
        /// the trace carries no atmosphere channel; we do not model a lapse rate here.</summary>
        public const double SpeedOfSoundSeaLevel = 340.29;

        public static double AltitudeMeters(FlightTrace trace, double t)
        {
            var p = trace.PositionAt(t);
            // Unity Y is ENU Up (see CoordinateBasis), i.e. altitude AGL in metres.
            return p.IsGap ? double.NaN : p.Value.y;
        }

        public static double SpeedMetersPerSecond(FlightTrace trace, double t)
        {
            var v = trace.VelocityAt(t);
            return v.IsGap ? double.NaN : v.Value.magnitude;
        }

        public static double VerticalSpeedMetersPerSecond(FlightTrace trace, double t)
        {
            var v = trace.VelocityAt(t);
            return v.IsGap ? double.NaN : v.Value.y;
        }

        public static double MachApprox(double speedMetersPerSecond) =>
            double.IsNaN(speedMetersPerSecond) ? double.NaN : speedMetersPerSecond / SpeedOfSoundSeaLevel;

        /// <summary>
        /// Active stage number: 1 (Terrier booster) until the first
        /// <c>StageIgnition</c> at or before <paramref name="t"/> lights the
        /// Black Brant sustainer, then 2. Derived purely from trace events.
        /// </summary>
        public static int StageNumber(IReadOnlyList<TraceEvent> events, double t)
        {
            int stage = 1;
            if (events == null)
                return stage;
            foreach (var e in events)
                if (e != null && e.Kind == "StageIgnition" && e.TimeSeconds <= t)
                    stage++;
            return Mathf.Clamp(stage, 1, 2);
        }

        public static string StageLabel(int stageNumber) =>
            stageNumber <= 1 ? "1 · Terrier booster" : "2 · Black Brant sustainer";

        /// <summary>
        /// Flight phase at <paramref name="t"/>, read from the trace's event
        /// stream and vertical-velocity sign. Never modelled — only classified.
        /// </summary>
        public static string Phase(FlightTrace trace, IReadOnlyList<TraceEvent> events, double t)
        {
            if (!HasEventAtOrBefore(events, "Liftoff", t))
                return "On pad";
            if (HasEventAtOrBefore(events, "Landing", t))
                return "Landed";
            if (PlumeController.IsPoweredAt(events, t))
                return "Powered ascent";

            bool descending = VerticalSpeedMetersPerSecond(trace, t) < 0.0;
            bool recovery = HasEventAtOrBefore(events, "RecoveryDeploy", t)
                || HasEventAtOrBefore(events, "MainDeploy", t);
            if (HasEventAtOrBefore(events, "Apogee", t) || descending)
                return recovery ? "Descent · recovery" : "Descent · ballistic";
            return "Coast · ascent";
        }

        private static bool HasEventAtOrBefore(IReadOnlyList<TraceEvent> events, string kind, double t)
        {
            if (events == null)
                return false;
            foreach (var e in events)
                if (e != null && e.Kind == kind && e.TimeSeconds <= t)
                    return true;
            return false;
        }

        /// <summary>The ordered State-panel readouts for review time <paramref name="t"/>.</summary>
        public static IReadOnlyList<StateReadout> StateAt(FlightTrace trace, double t)
        {
            var events = trace.Events;
            double alt = AltitudeMeters(trace, t);
            double speed = SpeedMetersPerSecond(trace, t);
            double mach = MachApprox(speed);
            int stage = StageNumber(events, t);
            // Switch altitude to km past 1 km so apogee stays legible; unit follows the value.
            bool altKm = !double.IsNaN(alt) && System.Math.Abs(alt) >= 1000.0;
            string altValue = double.IsNaN(alt) ? "—"
                : altKm ? (alt / 1000.0).ToString("0.00", CultureInfo.InvariantCulture)
                : alt.ToString("0", CultureInfo.InvariantCulture);

            return new[]
            {
                new StateReadout("Mission time", Fixed(t, 3), "s", EvidencePolicy.DirectFromTrace),
                new StateReadout("Altitude AGL", altValue, altKm ? "km" : "m", EvidencePolicy.DirectFromTrace),
                new StateReadout("Speed", Fixed(speed, 1), "m/s", EvidencePolicy.DirectFromTrace),
                new StateReadout("Mach", Fixed(mach, 2), "≈ M", EvidencePolicy.Approximate),
                new StateReadout("Stage", StageLabel(stage), "", EvidencePolicy.DerivedFromTrace),
                new StateReadout("Flight phase", Phase(trace, events, t), "", EvidencePolicy.DerivedFromTrace),
            };
        }

        private static string Fixed(double v, int decimals) =>
            double.IsNaN(v) ? "—" : v.ToString("0." + new string('0', decimals), CultureInfo.InvariantCulture);
    }
}
