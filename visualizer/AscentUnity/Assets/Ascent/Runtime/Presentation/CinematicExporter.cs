using System.Collections.Generic;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Describes one cinematic export. The manifest is written beside the output
    /// image sequence and fully attributes the render: the trace it came from,
    /// the protocol, the mission, the camera, the time window, and the evidence
    /// caveats. Export drives a presentation-only fixed clock and never mutates
    /// the accepted trace.
    /// </summary>
    public struct ExportRequest
    {
        public string TraceSha256;
        public string MissionId;
        public string CameraId;
        public double StartSeconds;
        public double EndSeconds;
        public string QualityPreset;
        public int Width;
        public int Height;
        public double FrameRate;
        public string UnityVersion;
        public IReadOnlyList<string> EvidenceCaveats;
    }

    public static class CinematicExporter
    {
        /// <summary>Build the canonical export manifest JSON for a request.</summary>
        public static string BuildManifest(ExportRequest r)
        {
            var manifest = new JObject
            {
                ["trace_sha256"] = r.TraceSha256,
                ["protocol_version"] = Bridge.Protocol.Version,
                ["mission_id"] = r.MissionId,
                ["camera_id"] = r.CameraId,
                ["start_seconds"] = r.StartSeconds,
                ["end_seconds"] = r.EndSeconds,
                ["quality_preset"] = r.QualityPreset,
                ["width"] = r.Width,
                ["height"] = r.Height,
                ["frame_rate"] = r.FrameRate,
                ["unity_version"] = r.UnityVersion,
                ["evidence_caveats"] = new JArray(r.EvidenceCaveats ?? new string[0]),
            };
            return manifest.ToString(Formatting.Indented);
        }

        /// <summary>The number of presentation frames the window spans at the given rate.</summary>
        public static int FrameCount(double startSeconds, double endSeconds, double frameRate)
        {
            if (endSeconds <= startSeconds || frameRate <= 0.0)
                return 0;
            return (int)System.Math.Round((endSeconds - startSeconds) * frameRate) + 1;
        }
    }
}
