using System.Collections.Generic;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using Ascent.Runtime.Bridge;

namespace Ascent.Runtime.Trace
{
    /// <summary>
    /// Recomputes the canonical trace SHA-256 the Rust bridge advertises. The
    /// canonical form is the exact serde_json encoding of
    /// <c>{"channels":[{"name","samples"}],"events":[{"kind","t_s","altitude_m","velocity_ms"}]}</c>
    /// with compact separators and shortest round-trip floats. A client match
    /// proves the stream reconstructed the trace byte-for-byte.
    /// </summary>
    public static class TraceHash
    {
        public struct Channel
        {
            public string Name;
            public double[] Samples;
        }

        public static string Compute(IReadOnlyList<Channel> channels, IReadOnlyList<TraceEvent> events)
        {
            var sb = new StringBuilder();
            sb.Append("{\"channels\":[");
            for (int i = 0; i < channels.Count; i++)
            {
                if (i > 0) sb.Append(',');
                sb.Append("{\"name\":");
                AppendJsonString(sb, channels[i].Name);
                sb.Append(",\"samples\":[");
                var s = channels[i].Samples;
                for (int j = 0; j < s.Length; j++)
                {
                    if (j > 0) sb.Append(',');
                    AppendDouble(sb, s[j]);
                }
                sb.Append("]}");
            }
            sb.Append("],\"events\":[");
            for (int i = 0; i < events.Count; i++)
            {
                if (i > 0) sb.Append(',');
                var e = events[i];
                sb.Append("{\"kind\":");
                AppendJsonString(sb, e.Kind);
                sb.Append(",\"t_s\":");
                AppendDouble(sb, e.TimeSeconds);
                sb.Append(",\"altitude_m\":");
                AppendDouble(sb, e.AltitudeMeters);
                sb.Append(",\"velocity_ms\":");
                AppendDouble(sb, e.VelocityMetersPerSecond);
                sb.Append('}');
            }
            sb.Append("]}");

            using (var sha = SHA256.Create())
            {
                var bytes = sha.ComputeHash(Encoding.UTF8.GetBytes(sb.ToString()));
                var hex = new StringBuilder(bytes.Length * 2);
                foreach (var b in bytes)
                    hex.Append(b.ToString("x2", CultureInfo.InvariantCulture));
                return hex.ToString();
            }
        }

        private static void AppendJsonString(StringBuilder sb, string value)
        {
            sb.Append('"');
            foreach (var c in value)
            {
                switch (c)
                {
                    case '"': sb.Append("\\\""); break;
                    case '\\': sb.Append("\\\\"); break;
                    case '\n': sb.Append("\\n"); break;
                    case '\r': sb.Append("\\r"); break;
                    case '\t': sb.Append("\\t"); break;
                    default: sb.Append(c); break;
                }
            }
            sb.Append('"');
        }

        // Shortest round-trip float, matching serde_json's float_roundtrip output:
        // a decimal point is always present for finite integral values.
        private static void AppendDouble(StringBuilder sb, double d)
        {
            string s = d.ToString("R", CultureInfo.InvariantCulture);
            if (s.IndexOf('.') < 0 && s.IndexOf('e') < 0 && s.IndexOf('E') < 0
                && s.IndexOf("inf") < 0 && s.IndexOf("NaN") < 0)
            {
                s += ".0";
            }
            sb.Append(s);
        }
    }
}
