using System;
using System.Globalization;
using System.IO;
using UnityEngine;
using UnityEngine.Rendering;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Packaged smoke-and-performance probe. In a standalone player launched with
    /// <c>-ascent-benchmark</c> it renders the review scene for a fixed window,
    /// measures the presented frame rate, writes a JSON record (path from
    /// <c>-ascent-out &lt;path&gt;</c>, else <see cref="Application.persistentDataPath"/>),
    /// logs a smoke line, and quits. It is inert in the editor and in any normal
    /// player launch, so committing it to the scene has no runtime effect there.
    /// This is the only honest way to record *packaged* FPS (the editor gate uses
    /// the in-editor PlayMode measurement); it holds no flight state.
    /// </summary>
    public sealed class PackagedBenchmark : MonoBehaviour
    {
        private const int WarmupFrames = 30;
        private const int SampleFrames = 300;
        private const double PackagedFpsThreshold = 45.0;

        private bool _armed;
        private string _outPath;
        private int _warm;
        private int _sampled;
        private double[] _frameMs;

        private void Start()
        {
            var args = Environment.GetCommandLineArgs();
            for (int i = 0; i < args.Length; i++)
            {
                if (args[i] == "-ascent-benchmark")
                    _armed = true;
                else if (args[i] == "-ascent-out" && i + 1 < args.Length)
                    _outPath = args[i + 1];
            }
            if (!_armed || Application.isEditor)
            {
                enabled = false;
                return;
            }
            if (string.IsNullOrEmpty(_outPath))
                _outPath = Path.Combine(Application.persistentDataPath, "packaged-performance.json");
            _frameMs = new double[SampleFrames];
            Screen.SetResolution(1920, 1080, false);
        }

        private void Update()
        {
            if (!_armed)
                return;
            if (_warm < WarmupFrames)
            {
                _warm++;
                return;
            }
            if (_sampled < SampleFrames)
            {
                _frameMs[_sampled++] = Time.unscaledDeltaTime * 1000.0;
                return;
            }
            WriteAndQuit();
        }

        private void WriteAndQuit()
        {
            _armed = false;
            Array.Sort(_frameMs);
            double medianMs = _frameMs[_frameMs.Length / 2];
            double p95Ms = _frameMs[(int)(_frameMs.Length * 0.95)];
            double medianFps = medianMs > 0.0 ? 1000.0 / medianMs : 0.0;
            bool hasGpu = SystemInfo.graphicsDeviceType != GraphicsDeviceType.Null;
            bool representative = hasGpu && medianFps < 1000.0;

            string json = "{\n"
                + Field("scene", "BlackBrantIX") + ",\n"
                + Field("kind", "packaged-standalone") + ",\n"
                + Field("intended_resolution", "1920x1080") + ",\n"
                + NumField("frames_sampled", _frameMs.Length) + ",\n"
                + NumField("median_fps", Math.Round(medianFps, 2)) + ",\n"
                + NumField("median_frame_ms", Math.Round(medianMs, 4)) + ",\n"
                + NumField("p95_frame_ms", Math.Round(p95Ms, 4)) + ",\n"
                + Field("graphics_device", SystemInfo.graphicsDeviceName) + ",\n"
                + Field("graphics_api", SystemInfo.graphicsDeviceType.ToString()) + ",\n"
                + BoolField("representative_render", representative) + ",\n"
                + NumField("packaged_fps_threshold", PackagedFpsThreshold) + ",\n"
                + BoolField("meets_packaged_threshold", representative && medianFps >= PackagedFpsThreshold) + ",\n"
                + Field("measured_utc", DateTime.UtcNow.ToString("o")) + "\n"
                + "}\n";

            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(_outPath));
                File.WriteAllText(_outPath, json);
                Debug.Log($"ASCENT_PACKAGED_SMOKE_OK median_fps={medianFps:0.0} out={_outPath}");
            }
            catch (Exception ex)
            {
                Debug.LogError($"ASCENT_PACKAGED_SMOKE_FAIL {ex.Message}");
            }
            Application.Quit(0);
        }

        private static string Field(string k, string v) =>
            $"  \"{k}\": {Newtonsoft.Json.JsonConvert.ToString(v ?? string.Empty)}";

        private static string NumField(string k, double v) =>
            $"  \"{k}\": {v.ToString("0.####", CultureInfo.InvariantCulture)}";

        private static string BoolField(string k, bool v) =>
            $"  \"{k}\": {(v ? "true" : "false")}";
    }
}
