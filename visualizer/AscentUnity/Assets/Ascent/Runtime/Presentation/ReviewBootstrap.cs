using System;
using System.IO;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Trace;
using UnityEngine;
using UnityEngine.UIElements;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// The single runtime driver of the Black Brant IX review. On scene start it
    /// launches the read-only Rust bridge, runs the reference mission to an
    /// accepted (hash-validated) <see cref="FlightTrace"/>, builds the
    /// <see cref="ReviewWorkbench"/> controller, and binds it to the live HUD — the
    /// trace is the only source of flight state. Once the trace is accepted the
    /// bridge is shut down (playback is pure seeking over an immutable trace), so
    /// no child process outlives trace acceptance. Space toggles play/pause; while
    /// playing it advances the review clock and drives the vehicle pose and exhaust
    /// plume purely from the trace.
    ///
    /// Launched with <c>-ascent-review-smoke</c> it self-verifies the whole path
    /// headlessly — bridge launch, accepted trace hash, HUD ready, the Space
    /// transport advancing playback, and clean shutdown leaving no child — then
    /// writes a JSON result, logs a smoke line, and quits.
    /// </summary>
    public sealed class ReviewBootstrap : MonoBehaviour
    {
        public const string MissionId = "nasa.black-brant-ix.reference";
        private const double FrameRate = 60.0;
        private const float ConnectTimeoutSeconds = 30f;
        private const int SmokeAdvanceFrames = 20;

        public SceneAnchors anchors;
        public ReviewHud hud;
        public PlumeController plume;

        private BridgeProcess _bridge;
        private MissionRunner _runner;
        private ReviewWorkbench _workbench;
        private float _deadline;
        private bool _bound;
        private bool _failed;

        // Smoke-mode state.
        private bool _smoke;
        private bool _smokeDone;
        private string _smokeOut;
        private double _smokeStartTime;
        private int _smokeFrames;

        private void Start()
        {
            ParseArgs();

            var path = BridgeLocator.Resolve();
            if (path == null)
            {
                Fail("bridge executable not found (StreamingAssets or target/debug)");
                return;
            }
            BridgeLocator.EnsureExecutable(path);

            try
            {
                // maxRestarts: 0 — the review runs one deterministic mission then
                // shuts the bridge down for good (playback is pure seeking). Auto
                // restart would misread our own intentional shutdown as a crash and
                // respawn a child, leaving an orphan; we never want that here.
                _bridge = new BridgeProcess(path, maxRestarts: 0);
                _bridge.Start();
                _runner = new MissionRunner(_bridge, MissionId);
                _runner.Begin();
                _deadline = Time.realtimeSinceStartup + ConnectTimeoutSeconds;
                SetLabel("status", "Connecting…");
            }
            catch (Exception ex)
            {
                Fail($"bridge launch failed: {ex.Message}");
            }
        }

        private void Update()
        {
            if (_failed || _smokeDone)
                return;

            if (!_bound)
            {
                PumpUntilReady();
                return;
            }

            // Ready: keyboard transport + trace-driven presentation.
            if (Input.GetKeyDown(KeyCode.Space))
                TogglePlay();

            _workbench.Clock.Advance(Time.deltaTime);
            ApplyTraceAt(_workbench.Clock.TimeSeconds);

            if (_smoke)
                DriveSmoke();
        }

        private void PumpUntilReady()
        {
            if (_runner == null)
                return;
            bool active = _runner.Pump();

            if (_runner.State == RunState.Ready)
            {
                OnTraceReady(_runner.Trace);
                return;
            }
            if (_runner.State == RunState.Failed)
            {
                Fail(_runner.LastError ?? "run failed");
                return;
            }
            if (!active && _runner.State != RunState.Ready)
            {
                Fail($"run ended without a trace (state {_runner.State})");
                return;
            }
            if (Time.realtimeSinceStartup > _deadline)
                Fail("bridge run timed out");
        }

        private void OnTraceReady(FlightTrace trace)
        {
            _workbench = new ReviewWorkbench(trace, FrameRate);
            if (hud != null)
                hud.SetWorkbench(_workbench);

            // The trace is fully in memory and immutable; the review replays by
            // seeking, so the live bridge is no longer needed. Shut it down now so
            // no child process outlives trace acceptance.
            ShutdownBridge();

            ApplyTraceAt(_workbench.Clock.TimeSeconds);
            SetLabel("trace-hash", $"trace {Short(trace.TraceHash, 16)}…");
            _bound = true;

            if (_smoke)
            {
                // Exercise the exact code path the Space key runs.
                TogglePlay();
                _smokeStartTime = _workbench.Clock.TimeSeconds;
            }
        }

        private void TogglePlay()
        {
            if (_workbench.Clock.IsPlaying) _workbench.Clock.Pause();
            else _workbench.Clock.Play();
        }

        /// <summary>Drive the vehicle pose and plume from the trace at time <paramref name="t"/>.</summary>
        private void ApplyTraceAt(double t)
        {
            var trace = _workbench.Trace;
            if (anchors != null && anchors.vehicleRoot != null)
            {
                var pos = trace.PositionAt(t);
                if (!pos.IsGap)
                    anchors.vehicleRoot.localPosition = pos.Value;

                // Point the stacked-along-+Y airframe along its velocity heading.
                var vel = trace.VelocityAt(t);
                if (!vel.IsGap && vel.Value.sqrMagnitude > 1e-6f)
                    anchors.vehicleRoot.localRotation =
                        Quaternion.FromToRotation(Vector3.up, vel.Value.normalized);
            }
            if (plume != null)
                plume.ApplyAt(trace.Events, t);
        }

        private void DriveSmoke()
        {
            _smokeFrames++;
            if (_smokeFrames < SmokeAdvanceFrames)
                return;
            bool advanced = _workbench.Clock.TimeSeconds > _smokeStartTime + 1e-6;
            bool noChild = _bridge == null || !_bridge.IsRunning;
            WriteSmoke(advanced && noChild, advanced, noChild);
        }

        private void Fail(string reason)
        {
            _failed = true;
            Debug.LogError($"ReviewBootstrap: {reason}");
            SetLabel("status", "Bridge error");
            SetLabel("trace-hash", reason);
            ShutdownBridge();
            if (_smoke && !_smokeDone)
                WriteSmoke(false, false, _bridge == null || !_bridge.IsRunning);
        }

        private void WriteSmoke(bool ok, bool advanced, bool noChild)
        {
            _smokeDone = true;
            string hash = _workbench?.Trace?.TraceHash ?? "";
            string json = "{\n"
                + "  \"kind\": \"review-smoke\",\n"
                + $"  \"mission_id\": \"{MissionId}\",\n"
                + $"  \"trace_sha256\": \"{hash}\",\n"
                + $"  \"trace_hash_len\": {hash.Length},\n"
                + $"  \"hud_ready\": {Bool(_bound)},\n"
                + $"  \"space_advances_playback\": {Bool(advanced)},\n"
                + $"  \"clean_shutdown_no_child\": {Bool(noChild)},\n"
                + $"  \"ok\": {Bool(ok)}\n"
                + "}\n";
            try
            {
                if (!string.IsNullOrEmpty(_smokeOut))
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(_smokeOut));
                    File.WriteAllText(_smokeOut, json);
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"review-smoke write failed: {ex.Message}");
            }

            if (ok)
                Debug.Log($"ASCENT_REVIEW_SMOKE_OK trace={Short(hash, 12)} advanced={advanced} noChild={noChild} out={_smokeOut}");
            else
                Debug.LogError($"ASCENT_REVIEW_SMOKE_FAIL ready={_bound} advanced={advanced} noChild={noChild}");

            Application.Quit(ok ? 0 : 7);
        }

        private void ShutdownBridge()
        {
            try { _bridge?.Shutdown(); } catch (Exception) { }
        }

        private void OnDestroy() => ShutdownBridge();

        private void ParseArgs()
        {
            var args = Environment.GetCommandLineArgs();
            for (int i = 0; i < args.Length; i++)
            {
                if (args[i] == "-ascent-review-smoke")
                    _smoke = true;
                else if (args[i] == "-ascent-out" && i + 1 < args.Length)
                    _smokeOut = args[i + 1];
            }
            if (_smoke && string.IsNullOrEmpty(_smokeOut))
                _smokeOut = Path.Combine(Application.persistentDataPath, "review-smoke.json");
        }

        private void SetLabel(string name, string text)
        {
            var doc = hud != null ? hud.GetComponent<UIDocument>() : null;
            var root = doc != null ? doc.rootVisualElement : null;
            var label = root?.Q<Label>(name);
            if (label != null)
                label.text = text;
        }

        private static string Bool(bool b) => b ? "true" : "false";

        private static string Short(string s, int n) =>
            string.IsNullOrEmpty(s) ? "" : s.Substring(0, Math.Min(n, s.Length));
    }
}
