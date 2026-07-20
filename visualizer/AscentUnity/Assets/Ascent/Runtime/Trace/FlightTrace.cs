using System.Collections.Generic;
using UnityEngine;

namespace Ascent.Runtime.Trace
{
    /// <summary>Result of sampling a trace at a time: a value plus whether the interval was valid.</summary>
    public readonly struct Sampled<T>
    {
        public readonly T Value;
        public readonly bool IsGap;

        public Sampled(T value, bool isGap)
        {
            Value = value;
            IsGap = isGap;
        }
    }

    /// <summary>
    /// An accepted, immutable flight trace. Once constructed it never changes;
    /// a failed reconstruction is discarded whole rather than partially applied.
    /// Canonical channels are East-North-Up doubles; conversion to Unity space
    /// happens only at sample time through <see cref="CoordinateBasis"/>.
    /// </summary>
    public sealed class FlightTrace
    {
        private readonly double[] _time;
        private readonly double[] _px, _py, _pz;
        private readonly double[] _vx, _vy, _vz;

        public string TraceHash { get; }
        public IReadOnlyList<Bridge.TraceEvent> Events { get; }
        public int SampleCount => _time.Length;
        public double StartSeconds => _time.Length > 0 ? _time[0] : 0.0;
        public double EndSeconds => _time.Length > 0 ? _time[_time.Length - 1] : 0.0;

        internal FlightTrace(
            double[] time,
            double[] px, double[] py, double[] pz,
            double[] vx, double[] vy, double[] vz,
            IReadOnlyList<Bridge.TraceEvent> events,
            string traceHash)
        {
            _time = time;
            _px = px; _py = py; _pz = pz;
            _vx = vx; _vy = vy; _vz = vz;
            Events = events;
            TraceHash = traceHash;
        }

        public Sampled<Vector3> PositionAt(double seconds)
        {
            return SampleVector(seconds, _px, _py, _pz);
        }

        public Sampled<Vector3> VelocityAt(double seconds)
        {
            return SampleVector(seconds, _vx, _vy, _vz);
        }

        /// <summary>
        /// Attitude derived from the velocity heading (the reference trace carries
        /// no independent attitude channel). Returns identity in a gap.
        /// </summary>
        public Sampled<Quaternion> AttitudeAt(double seconds)
        {
            var v = VelocityAt(seconds);
            if (v.IsGap || v.Value.sqrMagnitude < 1e-9f)
                return new Sampled<Quaternion>(Quaternion.identity, v.IsGap);
            return new Sampled<Quaternion>(Quaternion.LookRotation(v.Value.normalized, Vector3.up), false);
        }

        private Sampled<Vector3> SampleVector(double seconds, double[] cx, double[] cy, double[] cz)
        {
            if (_time.Length == 0)
                return new Sampled<Vector3>(Vector3.zero, true);
            if (seconds <= _time[0])
                return new Sampled<Vector3>(Convert(0, cx, cy, cz), false);
            if (seconds >= _time[_time.Length - 1])
                return new Sampled<Vector3>(Convert(_time.Length - 1, cx, cy, cz), false);

            int hi = LowerBound(seconds);
            int lo = hi - 1;
            double span = _time[hi] - _time[lo];
            double t = span > 0.0 ? (seconds - _time[lo]) / span : 0.0;
            var a = Convert(lo, cx, cy, cz);
            var b = Convert(hi, cx, cy, cz);
            return new Sampled<Vector3>(Vector3.Lerp(a, b, (float)t), false);
        }

        private static Vector3 Convert(int i, double[] cx, double[] cy, double[] cz)
        {
            return CoordinateBasis.Position(new Vector3d(cx[i], cy[i], cz[i]));
        }

        private int LowerBound(double seconds)
        {
            int lo = 0, hi = _time.Length - 1;
            while (lo < hi)
            {
                int mid = (lo + hi) / 2;
                if (_time[mid] < seconds) lo = mid + 1;
                else hi = mid;
            }
            return lo;
        }

        /// <summary>The exact trace time of the named event, or null if absent.</summary>
        public double? SeekEvent(string kind)
        {
            foreach (var e in Events)
                if (e.Kind == kind)
                    return e.TimeSeconds;
            return null;
        }
    }
}
