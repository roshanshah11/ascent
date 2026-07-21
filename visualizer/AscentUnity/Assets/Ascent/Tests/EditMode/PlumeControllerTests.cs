using System.Collections.Generic;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Presentation;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    /// <summary>
    /// The exhaust plume must be a faithful readout of the trace: powered only
    /// between a burn-start and the next burnout, across a two-stage flight.
    /// </summary>
    public class PlumeControllerTests
    {
        private static TraceEvent E(string kind, double t) => new TraceEvent { Kind = kind, TimeSeconds = t };

        // Terrier boost 0–6 s, coast to separation at 8 s, sustainer 8–40 s, apogee 150 s.
        private static List<TraceEvent> TwoStageFlight() => new List<TraceEvent>
        {
            E("Liftoff", 0.0),
            E("Burnout", 6.0),
            E("Burnout", 8.0),        // booster burnout coincident with...
            E("StageSeparation", 8.0),
            E("StageIgnition", 8.0),  // ...sustainer ignition at the same instant
            E("Burnout", 40.0),
            E("Apogee", 150.0),
        };

        [Test]
        public void PoweredDuringBoosterBurn()
        {
            var e = TwoStageFlight();
            Assert.That(PlumeController.IsPoweredAt(e, 0.0), Is.True, "at liftoff");
            Assert.That(PlumeController.IsPoweredAt(e, 3.0), Is.True, "mid boost");
        }

        [Test]
        public void UnpoweredDuringCoast()
        {
            Assert.That(PlumeController.IsPoweredAt(TwoStageFlight(), 7.0), Is.False, "coast between stages");
        }

        [Test]
        public void ReignitesAtCoincidentSeparation()
        {
            // Burnout and StageIgnition share t=8; the re-ignition must win so the
            // sustainer plume is lit immediately after separation.
            Assert.That(PlumeController.IsPoweredAt(TwoStageFlight(), 8.0), Is.True);
            Assert.That(PlumeController.IsPoweredAt(TwoStageFlight(), 20.0), Is.True, "mid sustainer");
        }

        [Test]
        public void UnpoweredAfterFinalBurnout()
        {
            var e = TwoStageFlight();
            Assert.That(PlumeController.IsPoweredAt(e, 40.001), Is.False, "just after burnout");
            Assert.That(PlumeController.IsPoweredAt(e, 150.0), Is.False, "at apogee");
        }

        [Test]
        public void UnpoweredBeforeLiftoffAndWithEmptyEvents()
        {
            Assert.That(PlumeController.IsPoweredAt(TwoStageFlight(), -1.0), Is.False, "before liftoff");
            Assert.That(PlumeController.IsPoweredAt(new List<TraceEvent>(), 5.0), Is.False, "no events");
            Assert.That(PlumeController.IsPoweredAt(null, 5.0), Is.False, "null events");
        }

        [Test]
        public void EmissionRateIsZeroWhenUnpowered()
        {
            Assert.That(PlumeController.EmissionRate(false, 800f), Is.EqualTo(0f));
            Assert.That(PlumeController.EmissionRate(true, 800f), Is.EqualTo(800f));
        }
    }
}
