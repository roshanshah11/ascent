using System.Collections;
using System.IO;
using System.Linq;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Presentation;
using Ascent.Runtime.Trace;
using Newtonsoft.Json.Linq;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

namespace Ascent.Tests.PlayMode
{
    /// <summary>
    /// Proves the review path end-to-end against the real Cargo-built bridge:
    /// launch → run Black Brant IX → accept an immutable trace → play, seek every
    /// named event, switch all five cameras, toggle layers, clean view → rerun
    /// with an identical hash → build the export manifest. Rendering, M3 FPS, and
    /// unguided review are out of scope for the automated check.
    /// </summary>
    public class VerticalSliceTests
    {
        private const string MissionId = "nasa.black-brant-ix.reference";

        private static string BridgePath =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../../../target/debug/ascent-visualizer-bridge"));

        private static void RequireBinary()
        {
            if (!File.Exists(BridgePath))
                Assert.Ignore($"bridge binary missing at {BridgePath}; run `cargo build -p ascent-visualizer-bridge`");
        }

        /// <summary>Run one mission on a fresh connection to an accepted trace.</summary>
        private static IEnumerator RunOnce(System.Action<MissionRunner> onDone)
        {
            var bridge = new BridgeProcess(BridgePath);
            bridge.Start();
            var runner = new MissionRunner(bridge, MissionId);
            runner.Begin();
            float deadline = Time.realtimeSinceStartup + 30f;
            while (runner.Pump() && Time.realtimeSinceStartup < deadline)
                yield return null;
            onDone(runner);
            bridge.Shutdown();
        }

        [UnityTest]
        public IEnumerator FullReviewPathPlaysSeeksCamerasLayersAndReruns()
        {
            RequireBinary();

            MissionRunner first = null;
            yield return RunOnce(r => first = r);
            Assert.That(first.State, Is.EqualTo(RunState.Ready), first.LastError);
            var trace = first.Trace;
            Assert.That(trace.TraceHash.Length, Is.EqualTo(64));
            Assert.That(trace.SampleCount, Is.GreaterThan(1));
            var kinds = trace.Events.Select(e => e.Kind).ToList();
            Assert.That(kinds.Any(k => k.Contains("Liftoff")), $"events: {string.Join(",", kinds)}");
            Assert.That(kinds.Any(k => k.Contains("Landing")), $"events: {string.Join(",", kinds)}");

            var workbench = new ReviewWorkbench(trace, 60.0);

            // Play ignition→landing by seeking every event to its exact Rust time.
            // Event kinds can repeat (booster + sustainer burnout), so seek each
            // instance by its timestamp rather than by kind.
            workbench.Clock.Play();
            foreach (var e in trace.Events)
            {
                workbench.Clock.Seek(e.TimeSeconds);
                Assert.That(workbench.Clock.TimeSeconds, Is.EqualTo(e.TimeSeconds).Within(1e-9), e.Kind);
            }
            // The named-event seek helper lands on some event of each distinct kind.
            foreach (var kind in trace.Events.Select(e => e.Kind).Distinct())
            {
                Assert.That(workbench.SeekEvent(kind), Is.True, kind);
                Assert.That(trace.Events.Any(e => e.Kind == kind
                    && System.Math.Abs(e.TimeSeconds - workbench.Clock.TimeSeconds) < 1e-9), Is.True, kind);
            }

            // Switching all five cameras must not move the review clock.
            var beforeCameras = workbench.Snapshot();
            foreach (var id in CameraDirector.Ids)
            {
                workbench.Cameras.SwitchTo(id);
                Assert.That(workbench.Clock.TimeSeconds, Is.EqualTo(beforeCameras.TimeSeconds).Within(1e-9), id);
            }

            // Clean view hides every layer without changing the rest of the state.
            var beforeClean = workbench.Snapshot();
            int shown = workbench.VisibleEngineeringLayerCount;
            Assert.That(shown, Is.GreaterThan(0));
            workbench.SetCleanView(true);
            Assert.That(workbench.VisibleEngineeringLayerCount, Is.Zero);
            Assert.That(workbench.Snapshot().EqualsIgnoringLayers(beforeClean), Is.True,
                "clean view changed something other than layer visibility");
            workbench.SetCleanView(false);
            Assert.That(workbench.VisibleEngineeringLayerCount, Is.EqualTo(shown), "layers not restored");

            // Rerun on a fresh connection: the trace hash is deterministic.
            MissionRunner second = null;
            yield return RunOnce(r => second = r);
            Assert.That(second.State, Is.EqualTo(RunState.Ready), second.LastError);
            Assert.That(second.Trace.TraceHash, Is.EqualTo(trace.TraceHash), "rerun hash differs");

            // Export manifest is fully attributed and points at this trace.
            var manifestJson = CinematicExporter.BuildManifest(new ExportRequest
            {
                TraceSha256 = trace.TraceHash,
                MissionId = MissionId,
                CameraId = CameraDirector.Chase,
                StartSeconds = trace.StartSeconds,
                EndSeconds = trace.EndSeconds,
                QualityPreset = "interactive",
                Width = 1920,
                Height = 1080,
                FrameRate = 60.0,
                UnityVersion = Application.unityVersion,
                EvidenceCaveats = new[] { "reference scenario, not a historical NASA flight" },
            });
            var manifest = JObject.Parse(manifestJson);
            Assert.That((string)manifest["trace_sha256"], Is.EqualTo(trace.TraceHash));
            Assert.That((string)manifest["mission_id"], Is.EqualTo(MissionId));

            // Persist the manifest to the gate's expected path so `cargo xtask
            // visualizer-test`'s export-manifest gate has a produced-on-disk artifact
            // carrying real trace provenance (not a synthetic fixture).
            var exportsDir = Path.GetFullPath(Path.Combine(Application.dataPath, "../Exports"));
            Directory.CreateDirectory(exportsDir);
            File.WriteAllText(Path.Combine(exportsDir, "manifest.json"), manifestJson);
        }

        [UnityTest]
        public IEnumerator RunCanBeCancelled()
        {
            RequireBinary();
            var bridge = new BridgeProcess(BridgePath);
            bridge.Start();
            var runner = new MissionRunner(bridge, MissionId);
            runner.Begin();

            // Cancel as soon as the run is active.
            float deadline = Time.realtimeSinceStartup + 30f;
            bool requested = false;
            while (Time.realtimeSinceStartup < deadline)
            {
                bool active = runner.Pump();
                if (!requested && runner.State == RunState.Running)
                {
                    runner.Cancel();
                    requested = true;
                }
                if (runner.State == RunState.Cancelled || runner.State == RunState.Ready)
                    break;
                if (!active && runner.State != RunState.Running)
                    break;
                yield return null;
            }
            Assert.That(requested, Is.True, "run never reached Running");
            Assert.That(runner.State, Is.EqualTo(RunState.Cancelled), $"state was {runner.State}");
            bridge.Shutdown();
        }
    }
}
