using System.Collections.Generic;
using System.Linq;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Presentation;
using Ascent.Runtime.Trace;
using NUnit.Framework;
using UnityEditor;
using UnityEngine.UIElements;

namespace Ascent.Tests.EditMode
{
    /// <summary>
    /// The review HUD binds the authored workbench UXML to the controller: one
    /// toggle per always-on engineering layer, one seek marker per trace event,
    /// and readout labels that mirror the controller snapshot. These exercise the
    /// pure tree-builders against an in-memory / real cloned UXML tree — no live
    /// panel required.
    /// </summary>
    public class ReviewHudTests
    {
        private const string UxmlPath = "Assets/Ascent/UI/ReviewWorkbench.uxml";

        private static readonly string[] AllChannels =
        {
            "time_s",
            "position_x_m", "position_y_m", "position_z_m",
            "velocity_x_ms", "velocity_y_ms", "velocity_z_ms",
        };

        private static long[] Bits(params double[] v) =>
            v.Select(System.BitConverter.DoubleToInt64Bits).ToArray();

        private static List<TraceEvent> TwoEvents() => new List<TraceEvent>
        {
            new TraceEvent { Kind = "Liftoff", TimeSeconds = 0.2 },
            new TraceEvent { Kind = "Landing", TimeSeconds = 1.8 },
        };

        private static FlightTrace BuildTrace(List<TraceEvent> events)
        {
            var data = new Dictionary<string, double[]>
            {
                ["time_s"] = new[] { 0.0, 1.0, 2.0 },
                ["position_x_m"] = new[] { 0.0, 10.0, 20.0 },
                ["position_y_m"] = new[] { 0.0, 0.0, 0.0 },
                ["position_z_m"] = new[] { 0.0, 100.0, 50.0 },
                ["velocity_x_ms"] = new[] { 10.0, 10.0, 10.0 },
                ["velocity_y_ms"] = new[] { 0.0, 0.0, 0.0 },
                ["velocity_z_ms"] = new[] { 100.0, 0.0, -50.0 },
            };
            var hashChannels = AllChannels
                .Select(n => new TraceHash.Channel { Name = n, SampleBits = Bits(data[n]) }).ToList();
            var manifest = new TraceManifestPayload
            {
                RunId = "run-1",
                TraceHash = TraceHash.Compute(hashChannels, events),
                SampleCount = 3,
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

        private static ReviewWorkbench NewWorkbench() => new ReviewWorkbench(BuildTrace(TwoEvents()), 60.0);

        [Test]
        public void LayerRailHasOneTogglePerAlwaysOnLayer()
        {
            var wb = NewWorkbench();
            var list = new VisualElement();

            int made = ReviewHud.PopulateLayerRail(list, wb, null);

            int expected = EngineeringLayerCatalog.Always().Count();
            Assert.That(expected, Is.GreaterThan(0), "catalog must declare at least one always-on layer");
            Assert.That(made, Is.EqualTo(expected));
            Assert.That(list.Query<Toggle>().ToList().Count, Is.EqualTo(expected));
        }

        [Test]
        public void LayerRailKeysEveryAlwaysOnLayerAndStartsVisible()
        {
            var wb = NewWorkbench();
            var list = new VisualElement();
            ReviewHud.PopulateLayerRail(list, wb, null);

            foreach (var layer in EngineeringLayerCatalog.Always())
            {
                var toggle = list.Q<Toggle>($"layer-{layer.Id}");
                Assert.That(toggle, Is.Not.Null, $"rail is missing a toggle for layer '{layer.Id}'");
                Assert.That(toggle.value, Is.True, $"layer '{layer.Id}' must start visible");
            }
        }

        [Test]
        public void EventTrackHasOneMarkerPerTraceEventInTimeOrder()
        {
            var trace = BuildTrace(TwoEvents());
            var track = new VisualElement();

            int made = ReviewHud.PopulateEventTrack(track, trace, null);

            Assert.That(made, Is.EqualTo(2));
            var markers = track.Query<Button>().ToList();
            Assert.That(markers.Count, Is.EqualTo(2));
            StringAssert.Contains("Liftoff", markers[0].text);
            StringAssert.Contains("Landing", markers[1].text);
        }

        [Test]
        public void EmptyTraceProducesNoMarkers()
        {
            var track = new VisualElement();
            Assert.That(ReviewHud.PopulateEventTrack(track, null, null), Is.EqualTo(0));
            Assert.That(track.childCount, Is.EqualTo(0));
        }

        [Test]
        public void RenderStateWritesReadoutsIntoTheAuthoredUxml()
        {
            var vta = AssetDatabase.LoadAssetAtPath<VisualTreeAsset>(UxmlPath);
            Assert.That(vta, Is.Not.Null, "authored workbench UXML must load");
            var root = new VisualElement();
            vta.CloneTree(root);
            var wb = NewWorkbench();

            ReviewHud.RenderState(root, wb);

            Assert.That(root.Q<Label>("clock"), Is.Not.Null, "uxml must carry a 'clock' label");
            Assert.That(root.Q<Label>("clock").text, Is.EqualTo("t = 0.000 s"));
            Assert.That(root.Q<Label>("rate").text, Is.EqualTo("1x"));
            StringAssert.Contains("Paused", root.Q<Label>("status").text);
        }
    }
}
