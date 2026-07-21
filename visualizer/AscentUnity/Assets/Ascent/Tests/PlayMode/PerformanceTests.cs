using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json.Linq;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.SceneManagement;
using UnityEngine.TestTools;

namespace Ascent.Tests.PlayMode
{
    /// <summary>
    /// Records an M3 frame-rate measurement for the Black Brant IX review scene and
    /// writes it to <c>TestResults/performance.json</c> as the platform-decision
    /// performance gate. The scene renders through its own camera on the real
    /// graphics device, so on an M-series Mac this is a genuine Metal measurement;
    /// the record captures the device and batchmode flags so its representativeness
    /// is explicit. It never hard-fails on the FPS number — the gate's job is to
    /// produce honest evidence, and the expand/contain rule is applied against it.
    /// </summary>
    public class PerformanceTests
    {
        private const string SceneName = "BlackBrantIX";
        private const int WarmupFrames = 30;
        private const int SampleFrames = 300;
        private const double EditorFpsThreshold = 30.0;

        private static string ResultsDir =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../TestResults"));

        [UnityTest]
        public IEnumerator RecordsReviewSceneFrameRateOnThisDevice()
        {
            // The scene must be registered in build settings (the builder does this).
            if (!Application.CanStreamedLevelBeLoaded(SceneName))
                Assert.Ignore($"scene '{SceneName}' not in build settings; run BuildFromBatch first");

            yield return SceneManager.LoadSceneAsync(SceneName, LoadSceneMode.Single);
            Screen.SetResolution(1920, 1080, false);

            // Let rendering settle before sampling.
            for (int i = 0; i < WarmupFrames; i++)
                yield return null;

            var frameMs = new List<double>(SampleFrames);
            for (int i = 0; i < SampleFrames; i++)
            {
                frameMs.Add(Time.unscaledDeltaTime * 1000.0);
                yield return null;
            }
            frameMs.Sort();

            double medianMs = frameMs[frameMs.Count / 2];
            double p95Ms = frameMs[(int)(frameMs.Count * 0.95)];
            double medianFps = medianMs > 0.0 ? 1000.0 / medianMs : 0.0;

            bool hasGpu = SystemInfo.graphicsDeviceType != GraphicsDeviceType.Null;
            // A batchmode run has no window/swapchain to present to, so the render
            // loop spins freely and reports absurd frame rates (thousands of FPS)
            // that do not reflect interactive performance. Only a windowed run that
            // actually presents frames is representative; guard on both signals.
            bool presenting = !Application.isBatchMode && medianFps < 1000.0;
            bool representative = hasGpu && presenting;
            string note = representative
                ? "windowed render presenting frames; interactive-editor measurement"
                : (Application.isBatchMode
                    ? "batchmode has no swapchain to present to — frame rate is a non-presenting artifact, NOT interactive"
                    : "frame rate implausibly high — not presenting to a display");

            var record = new JObject
            {
                ["scene"] = SceneName,
                ["intended_resolution"] = "1920x1080",
                ["frames_sampled"] = frameMs.Count,
                ["median_fps"] = Math.Round(medianFps, 2),
                ["median_frame_ms"] = Math.Round(medianMs, 4),
                ["p95_frame_ms"] = Math.Round(p95Ms, 4),
                ["graphics_device"] = SystemInfo.graphicsDeviceName,
                ["graphics_api"] = SystemInfo.graphicsDeviceType.ToString(),
                ["batchmode"] = Application.isBatchMode,
                ["representative_render"] = representative,
                ["representativeness_note"] = note,
                ["editor_fps_threshold"] = EditorFpsThreshold,
                ["meets_editor_threshold"] = representative && medianFps >= EditorFpsThreshold,
                ["measured_utc"] = DateTime.UtcNow.ToString("o"),
            };

            Directory.CreateDirectory(ResultsDir);
            var outPath = Path.Combine(ResultsDir, "performance.json");
            File.WriteAllText(outPath, record.ToString());
            Debug.Log($"PerformanceTests: wrote {outPath}\n{record}");

            // The gate produces evidence; it must not flake on hardware variance.
            Assert.That(File.Exists(outPath), Is.True, "performance record must be written");
            Assert.That(frameMs.Count, Is.EqualTo(SampleFrames), "must sample the full window");
        }
    }
}
