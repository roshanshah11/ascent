using Ascent.Runtime.Trace;

namespace Ascent.Runtime.Bridge
{
    public enum RunState
    {
        Idle,
        Negotiating,
        Running,
        Ready,
        Failed,
        Cancelled,
    }

    /// <summary>
    /// Drives one bridge run to an accepted, immutable <see cref="FlightTrace"/>.
    /// Pump it on the main thread each frame; it negotiates, requests the mission,
    /// assembles streamed chunks/events, and validates the canonical hash before
    /// publishing the trace. A failed run never replaces an existing trace.
    /// </summary>
    public sealed class MissionRunner
    {
        private readonly BridgeProcess _bridge;
        private readonly string _missionId;
        private TraceAssembler _assembler;
        private bool _helloSent;

        public MissionRunner(BridgeProcess bridge, string missionId)
        {
            _bridge = bridge;
            _missionId = missionId;
        }

        public RunState State { get; private set; } = RunState.Idle;
        public FlightTrace Trace { get; private set; }
        public string LastError { get; private set; }
        public string RunId { get; private set; }

        /// <summary>Begin: negotiate then request the mission.</summary>
        public void Begin()
        {
            _assembler = new TraceAssembler();
            State = RunState.Negotiating;
            _helloSent = true;
            _bridge.Send("hello", ClientRequest.Hello("ascent-unity"));
        }

        public void Cancel()
        {
            if (State == RunState.Running && RunId != null)
                _bridge.Send("cancel", ClientRequest.CancelRun(RunId));
        }

        /// <summary>Drain and process pending bridge messages. Returns true while active.</summary>
        public bool Pump()
        {
            while (_bridge.Poll(out var msg))
                Handle(msg.Envelope);
            return State == RunState.Negotiating || State == RunState.Running;
        }

        private void Handle(Envelope env)
        {
            switch (env.Kind)
            {
                case "hello_ack":
                    if (_helloSent)
                        _bridge.Send("run", ClientRequest.RunMission(_missionId));
                    break;
                case "run_started":
                    RunId = env.PayloadAs<RunStartedProbe>().RunId;
                    State = RunState.Running;
                    break;
                case "trace_manifest":
                    _assembler.AcceptManifest(env.PayloadAs<TraceManifestPayload>());
                    break;
                case "trace_channel_chunk":
                    _assembler.AcceptChunk(env.PayloadAs<TraceChannelChunkPayload>());
                    break;
                case "trace_events":
                    _assembler.AcceptEvents(env.PayloadAs<TraceEventsPayload>());
                    break;
                case "run_completed":
                    try
                    {
                        Trace = _assembler.Build();
                        State = RunState.Ready;
                    }
                    catch (TraceRejected ex)
                    {
                        LastError = ex.Message;
                        State = RunState.Failed;
                    }
                    break;
                case "run_cancelled":
                    State = RunState.Cancelled;
                    break;
                case "error":
                    LastError = env.PayloadAs<ErrorPayload>().Message;
                    State = RunState.Failed;
                    break;
            }
        }

        private sealed class RunStartedProbe
        {
            [Newtonsoft.Json.JsonProperty("run_id")] public string RunId { get; set; }
        }
    }
}
