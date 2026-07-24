using System.Collections.Generic;
using System.Linq;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Presentation;
using Ascent.Runtime.Trace;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    /// <summary>
    /// The State panel reads its numbers from the trace, never from a simulation
    /// in Unity. These pin the derivations: altitude/speed straight from the
    /// channels, stage/phase classified from the event stream, and Mach graded
    /// Approximate.
    /// </summary>
    public class FlightReadoutTests
    {
        private static readonly string[] AllChannels =
        {
            "time_s",
            "position_x_m", "position_y_m", "position_z_m",
            "velocity_x_ms", "velocity_y_ms", "velocity_z_ms",
        };

        private static long[] Bits(params double[] v) =>
            v.Select(System.BitConverter.DoubleToInt64Bits).ToArray();

        // ENU (E,N,U) -> Unity (E,U,N): CoordinateBasis maps (x,y,z)->(x,z,y), so the
        // ENU Up channel is position_z_m / velocity_z_ms and that becomes Unity Y
        // (altitude / vertical speed). Altitude and vertical velocity therefore live
        // in the *_z_ channels here, not the *_y_ ones.
        private static FlightTrace Build(List<TraceEvent> events)
        {
            var data = new Dictionary<string, double[]>
            {
                ["time_s"] = new[] { 0.0, 1.0, 2.0, 3.0 },
                ["position_x_m"] = new[] { 0.0, 0.0, 0.0, 0.0 },
                ["position_y_m"] = new[] { 0.0, 0.0, 0.0, 0.0 },   // North
                ["position_z_m"] = new[] { 0.0, 50.0, 150.0, 120.0 }, // Up = altitude, climbs then falls
                ["velocity_x_ms"] = new[] { 0.0, 0.0, 0.0, 0.0 },
                ["velocity_y_ms"] = new[] { 0.0, 0.0, 0.0, 0.0 },   // North
                ["velocity_z_ms"] = new[] { 60.0, 80.0, 0.0, -40.0 }, // Up: climbing, apogee, descending
            };
            var hashChannels = AllChannels
                .Select(n => new TraceHash.Channel { Name = n, SampleBits = Bits(data[n]) }).ToList();
            var manifest = new TraceManifestPayload
            {
                RunId = "run-1",
                TraceHash = TraceHash.Compute(hashChannels, events),
                SampleCount = 4,
                EventCount = events.Count,
                Channels = AllChannels.Select(n => new ChannelSpec { Name = n, SampleCount = data[n].Length }).ToList(),
            };
            var asm = new TraceAssembler();
            asm.AcceptManifest(manifest);
            foreach (var n in AllChannels)
                asm.AcceptChunk(new TraceChannelChunkPayload { RunId = "run-1", Channel = n, Sequence = 0, SampleBits = Bits(data[n]) });
            asm.AcceptEvents(new TraceEventsPayload { RunId = "run-1", Events = events });
            return asm.Build();
        }

        private static List<TraceEvent> FullFlight() => new List<TraceEvent>
        {
            new TraceEvent { Kind = "Liftoff", TimeSeconds = 0.2 },
            new TraceEvent { Kind = "Burnout", TimeSeconds = 0.9 },
            new TraceEvent { Kind = "StageSeparation", TimeSeconds = 1.0 },
            new TraceEvent { Kind = "StageIgnition", TimeSeconds = 1.05 },
            new TraceEvent { Kind = "Burnout", TimeSeconds = 1.9 }, // sustainer burns out before apogee
            new TraceEvent { Kind = "Apogee", TimeSeconds = 2.0 },
            new TraceEvent { Kind = "Landing", TimeSeconds = 3.0 },
        };

        [Test]
        public void AltitudeAndSpeedComeStraightFromTheChannels()
        {
            var trace = Build(FullFlight());
            Assert.That(FlightReadout.AltitudeMeters(trace, 1.0), Is.EqualTo(50.0).Within(1e-6));
            Assert.That(FlightReadout.SpeedMetersPerSecond(trace, 1.0), Is.EqualTo(80.0).Within(1e-6));
            Assert.That(FlightReadout.VerticalSpeedMetersPerSecond(trace, 3.0), Is.EqualTo(-40.0).Within(1e-6));
        }

        [Test]
        public void MachIsSpeedOverSeaLevelSoundSpeed()
        {
            Assert.That(FlightReadout.MachApprox(FlightReadout.SpeedOfSoundSeaLevel),
                Is.EqualTo(1.0).Within(1e-9));
        }

        [Test]
        public void StageNumberFollowsSustainerIgnition()
        {
            var events = FullFlight();
            Assert.That(FlightReadout.StageNumber(events, 0.5), Is.EqualTo(1));
            Assert.That(FlightReadout.StageNumber(events, 1.5), Is.EqualTo(2));
        }

        [Test]
        public void PhaseIsClassifiedFromEventsAndVerticalVelocity()
        {
            var trace = Build(FullFlight());
            var events = trace.Events;
            Assert.That(FlightReadout.Phase(trace, events, 0.0), Is.EqualTo("On pad"));
            Assert.That(FlightReadout.Phase(trace, events, 0.5), Is.EqualTo("Powered ascent"));
            Assert.That(FlightReadout.Phase(trace, events, 2.5), Does.StartWith("Descent"));
            Assert.That(FlightReadout.Phase(trace, events, 3.0), Is.EqualTo("Landed"));
        }

        [Test]
        public void StateAtCarriesUnitsAndGradedProvenance()
        {
            var trace = Build(FullFlight());
            var rows = FlightReadout.StateAt(trace, 1.0);
            Assert.That(rows.Count, Is.EqualTo(6));
            foreach (var r in rows)
            {
                Assert.That(r.Label, Is.Not.Empty);
                Assert.That(r.Provenance, Is.Not.EqualTo(EvidencePolicy.Unspecified), r.Label);
            }
            var mach = rows.First(r => r.Label == "Mach");
            Assert.That(mach.Provenance, Is.EqualTo(EvidencePolicy.Approximate));
        }
    }
}
