using System.Collections.Generic;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Ascent.Runtime.Bridge
{
    /// <summary>
    /// Visualizer protocol v1 constants and message shapes. These mirror the
    /// Rust `ascent-visualizer-protocol` crate exactly; the cross-language
    /// fixtures under <c>data/protocol/visualizer/v1</c> are the shared oracle.
    /// </summary>
    public static class Protocol
    {
        public const ushort Version = 1;
        public const int MaxFrameBytes = 16 * 1024 * 1024;
        public const int MaxChunkSamples = 1024;
    }

    /// <summary>A decoded frame: fixed envelope fields plus a raw payload token.</summary>
    public sealed class Envelope
    {
        [JsonProperty("protocol_version")] public ushort ProtocolVersion { get; set; }
        [JsonProperty("message_id")] public string MessageId { get; set; }
        [JsonProperty("request_id")] public string RequestId { get; set; }
        [JsonProperty("kind")] public string Kind { get; set; }
        [JsonProperty("payload")] public JObject Payload { get; set; }

        public T PayloadAs<T>() => Payload.ToObject<T>();
    }

    /// <summary>Client → bridge request payloads. Serialized with an internal "kind" tag.</summary>
    public static class ClientRequest
    {
        public static JObject Hello(string client) => new JObject
        {
            ["kind"] = "hello",
            ["client"] = client,
            ["protocol_version"] = Protocol.Version,
        };

        public static JObject ListMissions() => new JObject { ["kind"] = "list_missions" };

        public static JObject RunMission(string missionId) => new JObject
        {
            ["kind"] = "run_mission",
            ["mission_id"] = missionId,
        };

        public static JObject CancelRun(string runId) => new JObject
        {
            ["kind"] = "cancel_run",
            ["run_id"] = runId,
        };

        public static JObject Shutdown() => new JObject { ["kind"] = "shutdown" };
    }

    // Server payload projections used by the assembler.

    public sealed class MissionEntry
    {
        [JsonProperty("mission_id")] public string MissionId { get; set; }
        [JsonProperty("title")] public string Title { get; set; }
    }

    public sealed class ChannelSpec
    {
        [JsonProperty("name")] public string Name { get; set; }
        [JsonProperty("sample_count")] public int SampleCount { get; set; }
    }

    public sealed class TraceEvent
    {
        [JsonProperty("kind")] public string Kind { get; set; }
        [JsonProperty("t_s")] public double TimeSeconds { get; set; }
        [JsonProperty("altitude_m")] public double AltitudeMeters { get; set; }
        [JsonProperty("velocity_ms")] public double VelocityMetersPerSecond { get; set; }
    }

    public sealed class MissionCatalogPayload
    {
        [JsonProperty("missions")] public List<MissionEntry> Missions { get; set; }
    }

    public sealed class TraceManifestPayload
    {
        [JsonProperty("run_id")] public string RunId { get; set; }
        [JsonProperty("trace_hash")] public string TraceHash { get; set; }
        [JsonProperty("sample_count")] public int SampleCount { get; set; }
        [JsonProperty("event_count")] public int EventCount { get; set; }
        [JsonProperty("channels")] public List<ChannelSpec> Channels { get; set; }
    }

    public sealed class TraceChannelChunkPayload
    {
        [JsonProperty("run_id")] public string RunId { get; set; }
        [JsonProperty("channel")] public string Channel { get; set; }
        [JsonProperty("sequence")] public uint Sequence { get; set; }
        [JsonProperty("samples")] public double[] Samples { get; set; }
    }

    public sealed class TraceEventsPayload
    {
        [JsonProperty("run_id")] public string RunId { get; set; }
        [JsonProperty("events")] public List<TraceEvent> Events { get; set; }
    }

    public sealed class RunCompletedPayload
    {
        [JsonProperty("run_id")] public string RunId { get; set; }
        [JsonProperty("trace_hash")] public string TraceHash { get; set; }
    }

    public sealed class ErrorPayload
    {
        [JsonProperty("code")] public string Code { get; set; }
        [JsonProperty("message")] public string Message { get; set; }
    }
}
