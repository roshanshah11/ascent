using System;
using System.Collections.Generic;
using Ascent.Runtime.Bridge;

namespace Ascent.Runtime.Trace
{
    /// <summary>Raised when a streamed trace fails validation; the current mission is left untouched.</summary>
    public sealed class TraceRejected : Exception
    {
        public TraceRejected(string message) : base(message) { }
    }

    /// <summary>
    /// Reassembles a streamed trace: collects channel chunks (contiguous
    /// sequences from zero), checks declared counts, requires every declared
    /// channel and the event set, and validates the canonical SHA-256 against
    /// the manifest. Only a fully valid trace becomes a <see cref="FlightTrace"/>.
    /// </summary>
    public sealed class TraceAssembler
    {
        private static readonly string[] Required =
        {
            "time_s",
            "position_x_m", "position_y_m", "position_z_m",
            "velocity_x_ms", "velocity_y_ms", "velocity_z_ms",
        };

        private TraceManifestPayload _manifest;
        private readonly Dictionary<string, List<long>> _channels = new Dictionary<string, List<long>>();
        private readonly Dictionary<string, uint> _nextSeq = new Dictionary<string, uint>();
        private List<TraceEvent> _events;

        public void AcceptManifest(TraceManifestPayload manifest)
        {
            _manifest = manifest ?? throw new TraceRejected("null manifest");
        }

        public void AcceptChunk(TraceChannelChunkPayload chunk)
        {
            if (_manifest == null)
                throw new TraceRejected("chunk before manifest");
            if (chunk.SampleBits.Length > Protocol.MaxChunkSamples)
                throw new TraceRejected($"chunk for {chunk.Channel} exceeds {Protocol.MaxChunkSamples} samples");

            if (!_channels.TryGetValue(chunk.Channel, out var list))
            {
                list = new List<long>();
                _channels[chunk.Channel] = list;
                _nextSeq[chunk.Channel] = 0;
            }
            if (chunk.Sequence != _nextSeq[chunk.Channel])
                throw new TraceRejected(
                    $"chunk for {chunk.Channel} out of order: got {chunk.Sequence}, expected {_nextSeq[chunk.Channel]}");
            _nextSeq[chunk.Channel] = chunk.Sequence + 1;
            list.AddRange(chunk.SampleBits);
        }

        public void AcceptEvents(TraceEventsPayload events)
        {
            _events = events.Events ?? new List<TraceEvent>();
        }

        /// <summary>Finalize into an immutable trace, or throw <see cref="TraceRejected"/>.</summary>
        public FlightTrace Build()
        {
            if (_manifest == null)
                throw new TraceRejected("no manifest");
            if (_events == null)
                throw new TraceRejected("no events");
            if (_events.Count != _manifest.EventCount)
                throw new TraceRejected($"event count mismatch: {_events.Count} != {_manifest.EventCount}");

            // Each declared channel must be complete at its declared length.
            foreach (var spec in _manifest.Channels)
            {
                if (!_channels.TryGetValue(spec.Name, out var list))
                    throw new TraceRejected($"missing channel {spec.Name}");
                if (list.Count != spec.SampleCount)
                    throw new TraceRejected(
                        $"channel {spec.Name} length {list.Count} != declared {spec.SampleCount}");
            }
            foreach (var name in Required)
                if (!_channels.ContainsKey(name))
                    throw new TraceRejected($"required channel {name} absent");

            // Recompute the canonical hash in manifest channel order.
            var hashChannels = new List<TraceHash.Channel>();
            foreach (var spec in _manifest.Channels)
                hashChannels.Add(new TraceHash.Channel { Name = spec.Name, SampleBits = _channels[spec.Name].ToArray() });
            var computed = TraceHash.Compute(hashChannels, _events);
            if (!string.Equals(computed, _manifest.TraceHash, StringComparison.Ordinal))
                throw new TraceRejected($"trace hash mismatch: {computed} != {_manifest.TraceHash}");

            return new FlightTrace(
                ToDoubles(_channels["time_s"]),
                ToDoubles(_channels["position_x_m"]),
                ToDoubles(_channels["position_y_m"]),
                ToDoubles(_channels["position_z_m"]),
                ToDoubles(_channels["velocity_x_ms"]),
                ToDoubles(_channels["velocity_y_ms"]),
                ToDoubles(_channels["velocity_z_ms"]),
                _events,
                _manifest.TraceHash);
        }

        private static double[] ToDoubles(List<long> bits)
        {
            var values = new double[bits.Count];
            for (var i = 0; i < bits.Count; i++)
                values[i] = BitConverter.Int64BitsToDouble(bits[i]);
            return values;
        }
    }
}
