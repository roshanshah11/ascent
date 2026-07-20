using System.Collections.Generic;
using System.Linq;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Trace;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    public class TraceAssemblerTests
    {
        // The exact channel/event values baked into the Rust-generated fixtures.
        private static List<TraceHash.Channel> FixtureChannels() => new List<TraceHash.Channel>
        {
            new TraceHash.Channel { Name = "time_s", Samples = new[] { 0.0, 0.02, 0.04 } },
            new TraceHash.Channel { Name = "position_z_m", Samples = new[] { 0.0, 1.5, 6.0 } },
        };

        private static List<TraceEvent> FixtureEvents() => new List<TraceEvent>
        {
            new TraceEvent { Kind = "Liftoff", TimeSeconds = 0.2, AltitudeMeters = 0.0, VelocityMetersPerSecond = 12.0 },
            new TraceEvent { Kind = "Landing", TimeSeconds = 3900.0, AltitudeMeters = 0.0, VelocityMetersPerSecond = 8.0 },
        };

        private static Envelope DecodeSingle(string rel)
        {
            var bytes = Fixtures.ReadFrame(rel);
            var decoder = new FrameDecoder(16 * 1024 * 1024);
            return decoder.Push(bytes).Single();
        }

        [Test]
        public void ClientHashMatchesRustManifestFixture()
        {
            var manifest = DecodeSingle("server/trace_manifest.frame").PayloadAs<TraceManifestPayload>();
            var computed = TraceHash.Compute(FixtureChannels(), FixtureEvents());
            Assert.That(computed, Is.EqualTo(manifest.TraceHash),
                "C# canonical hash must match the Rust bridge's advertised hash");
        }

        [Test]
        public void HashInvalidManifestFixtureIsRejected()
        {
            var manifest = DecodeSingle("server/trace_manifest_hash_invalid.frame").PayloadAs<TraceManifestPayload>();
            var computed = TraceHash.Compute(FixtureChannels(), FixtureEvents());
            Assert.That(manifest.TraceHash, Is.Not.EqualTo(computed));
        }

        // --- assembler validation over a self-consistent synthetic trace ---

        private static readonly string[] AllChannels =
        {
            "time_s",
            "position_x_m", "position_y_m", "position_z_m",
            "velocity_x_ms", "velocity_y_ms", "velocity_z_ms",
        };

        private static Dictionary<string, double[]> SyntheticData() => new Dictionary<string, double[]>
        {
            ["time_s"] = new[] { 0.0, 1.0, 2.0 },
            ["position_x_m"] = new[] { 0.0, 10.0, 20.0 },
            ["position_y_m"] = new[] { 0.0, 0.0, 0.0 },
            ["position_z_m"] = new[] { 0.0, 100.0, 50.0 },
            ["velocity_x_ms"] = new[] { 10.0, 10.0, 10.0 },
            ["velocity_y_ms"] = new[] { 0.0, 0.0, 0.0 },
            ["velocity_z_ms"] = new[] { 100.0, 0.0, -50.0 },
        };

        private static TraceManifestPayload SyntheticManifest(Dictionary<string, double[]> data, List<TraceEvent> events, string hashOverride = null)
        {
            var hashChannels = AllChannels.Select(n => new TraceHash.Channel { Name = n, Samples = data[n] }).ToList();
            return new TraceManifestPayload
            {
                RunId = "run-1",
                TraceHash = hashOverride ?? TraceHash.Compute(hashChannels, events),
                SampleCount = 3,
                EventCount = events.Count,
                Channels = AllChannels.Select(n => new ChannelSpec { Name = n, SampleCount = data[n].Length }).ToList(),
            };
        }

        private static void FeedChunks(TraceAssembler asm, Dictionary<string, double[]> data)
        {
            foreach (var name in AllChannels)
                asm.AcceptChunk(new TraceChannelChunkPayload { RunId = "run-1", Channel = name, Sequence = 0, Samples = data[name] });
        }

        [Test]
        public void AssemblerBuildsAndSamplesAValidTrace()
        {
            var data = SyntheticData();
            var events = FixtureEvents();
            var asm = new TraceAssembler();
            asm.AcceptManifest(SyntheticManifest(data, events));
            FeedChunks(asm, data);
            asm.AcceptEvents(new TraceEventsPayload { RunId = "run-1", Events = events });

            var trace = asm.Build();
            Assert.That(trace.SampleCount, Is.EqualTo(3));
            Assert.That(trace.SeekEvent("Landing"), Is.EqualTo(3900.0));
            // Halfway between t=0 and t=1: East position lerps 0→10.
            var pos = trace.PositionAt(0.5);
            Assert.That(pos.IsGap, Is.False);
            Assert.That(pos.Value.x, Is.EqualTo(5.0).Within(1e-4));
        }

        [Test]
        public void AssemblerRejectsOutOfOrderChunk()
        {
            var asm = new TraceAssembler();
            asm.AcceptManifest(SyntheticManifest(SyntheticData(), FixtureEvents()));
            Assert.Throws<TraceRejected>(() =>
                asm.AcceptChunk(new TraceChannelChunkPayload { RunId = "run-1", Channel = "time_s", Sequence = 1, Samples = new[] { 0.0 } }));
        }

        [Test]
        public void AssemblerRejectsMissingChannel()
        {
            var data = SyntheticData();
            var asm = new TraceAssembler();
            asm.AcceptManifest(SyntheticManifest(data, FixtureEvents()));
            // Feed all but the last required channel.
            foreach (var name in AllChannels.Take(AllChannels.Length - 1))
                asm.AcceptChunk(new TraceChannelChunkPayload { RunId = "run-1", Channel = name, Sequence = 0, Samples = data[name] });
            asm.AcceptEvents(new TraceEventsPayload { RunId = "run-1", Events = FixtureEvents() });
            Assert.Throws<TraceRejected>(() => asm.Build());
        }

        [Test]
        public void AssemblerRejectsHashMismatch()
        {
            var data = SyntheticData();
            var events = FixtureEvents();
            var asm = new TraceAssembler();
            asm.AcceptManifest(SyntheticManifest(data, events, hashOverride: new string('0', 64)));
            FeedChunks(asm, data);
            asm.AcceptEvents(new TraceEventsPayload { RunId = "run-1", Events = events });
            Assert.Throws<TraceRejected>(() => asm.Build());
        }

        [Test]
        public void AssemblerRejectsEventCountMismatch()
        {
            var data = SyntheticData();
            var events = FixtureEvents();
            var asm = new TraceAssembler();
            asm.AcceptManifest(SyntheticManifest(data, events));
            FeedChunks(asm, data);
            asm.AcceptEvents(new TraceEventsPayload { RunId = "run-1", Events = new List<TraceEvent> { events[0] } });
            Assert.Throws<TraceRejected>(() => asm.Build());
        }
    }
}
