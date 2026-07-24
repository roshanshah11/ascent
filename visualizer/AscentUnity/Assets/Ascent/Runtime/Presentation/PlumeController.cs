using System.Collections.Generic;
using Ascent.Runtime.Bridge;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Drives an exhaust <see cref="ParticleSystem"/> from the accepted flight
    /// trace: the plume emits only during powered flight and its intensity
    /// scales with how hard the active stage is burning. The powered state is
    /// derived purely from trace events — never from Unity physics — so the
    /// visual stays a faithful readout of the simulation.
    ///
    /// A real VFX-Graph plume can be swapped in later; the binding contract
    /// (powered interval + emission rate) lives in the pure helpers below and
    /// is unit-tested without a live particle system.
    /// </summary>
    [RequireComponent(typeof(ParticleSystem))]
    public sealed class PlumeController : MonoBehaviour
    {
        [Tooltip("Particle emission rate (per second) at full burn.")]
        public float maxEmissionRate = 800f;

        [Tooltip("Particle start speed (m/s) at full burn.")]
        public float maxStartSpeed = 45f;

        [Tooltip("HDRP-safe mesh plume toggled from the same trace-derived powered state.")]
        public Renderer plumeCore;

        private ParticleSystem _system;

        private void Awake() => _system = GetComponent<ParticleSystem>();

        /// <summary>Applies the trace-derived burn state for time <paramref name="t"/>.</summary>
        public void ApplyAt(IReadOnlyList<TraceEvent> events, double t)
        {
            if (_system == null)
                _system = GetComponent<ParticleSystem>();
            bool powered = IsPoweredAt(events, t);
            if (plumeCore != null)
                plumeCore.enabled = powered;
            var emission = _system.emission;
            emission.rateOverTime = EmissionRate(powered, maxEmissionRate);
            var main = _system.main;
            main.startSpeed = powered ? maxStartSpeed : 0f;
            if (powered && !_system.isEmitting)
                _system.Play();
            else if (!powered && _system.isEmitting)
                _system.Stop(true, ParticleSystemStopBehavior.StopEmitting);
        }

        /// <summary>
        /// True when the vehicle is under thrust at time <paramref name="t"/>:
        /// the most recent burn-boundary event at or before t is a burn-start
        /// (<c>Liftoff</c> or <c>StageIgnition</c>) rather than a <c>Burnout</c>.
        /// Events need not be pre-sorted.
        /// </summary>
        public static bool IsPoweredAt(IReadOnlyList<TraceEvent> events, double t)
        {
            if (events == null)
                return false;
            bool powered = false;
            double lastBoundary = double.NegativeInfinity;
            foreach (var e in events)
            {
                if (e == null || e.TimeSeconds > t)
                    continue;
                bool isStart = e.Kind == "Liftoff" || e.Kind == "StageIgnition";
                bool isEnd = e.Kind == "Burnout";
                if (!isStart && !isEnd)
                    continue;
                // Take the latest boundary at or before t. Ties resolve to the
                // burn-end (separation emits Burnout then StageIgnition at the
                // same instant, so a coincident start re-lights immediately).
                if (e.TimeSeconds >= lastBoundary)
                {
                    if (e.TimeSeconds > lastBoundary)
                    {
                        powered = isStart;
                        lastBoundary = e.TimeSeconds;
                    }
                    else if (isStart)
                    {
                        powered = true; // coincident re-ignition wins the tie
                    }
                }
            }
            return powered;
        }

        /// <summary>Emission rate for the current burn state.</summary>
        public static float EmissionRate(bool powered, float maxRate) => powered ? maxRate : 0f;
    }
}
