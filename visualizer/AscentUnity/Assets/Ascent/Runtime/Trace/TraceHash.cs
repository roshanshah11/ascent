using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using Ascent.Runtime.Bridge;

namespace Ascent.Runtime.Trace
{
    /// <summary>
    /// Recomputes the canonical trace SHA-256 the Rust bridge advertises. The
    /// canonical form is a binary byte stream (not JSON): floats are hashed as
    /// their little-endian IEEE-754 bit patterns, so Rust and C# agree without
    /// depending on language-specific float formatting. A client match proves the
    /// stream reconstructed the trace.
    ///
    /// Layout (all integers little-endian):
    ///   u32 channel_count
    ///     per channel: u32 name_len, name bytes, u32 sample_count, f64-LE * count
    ///   u32 event_count
    ///     per event: u32 kind_len, kind bytes, f64-LE t_s, altitude_m, velocity_ms
    /// </summary>
    public static class TraceHash
    {
        public struct Channel
        {
            public string Name;
            public long[] SampleBits;
        }

        public static string Compute(IReadOnlyList<Channel> channels, IReadOnlyList<TraceEvent> events)
        {
            using (var ms = new MemoryStream())
            {
                WriteU32(ms, (uint)channels.Count);
                foreach (var c in channels)
                {
                    var name = Encoding.UTF8.GetBytes(c.Name);
                    WriteU32(ms, (uint)name.Length);
                    ms.Write(name, 0, name.Length);
                    WriteU32(ms, (uint)c.SampleBits.Length);
                    foreach (var bits in c.SampleBits)
                        WriteI64(ms, bits);
                }
                WriteU32(ms, (uint)events.Count);
                foreach (var e in events)
                {
                    var kind = Encoding.UTF8.GetBytes(e.Kind);
                    WriteU32(ms, (uint)kind.Length);
                    ms.Write(kind, 0, kind.Length);
            WriteI64(ms, BitConverter.DoubleToInt64Bits(e.TimeSeconds));
            WriteI64(ms, BitConverter.DoubleToInt64Bits(e.AltitudeMeters));
            WriteI64(ms, BitConverter.DoubleToInt64Bits(e.VelocityMetersPerSecond));
                }

                ms.Position = 0;
                using (var sha = SHA256.Create())
                {
                    var digest = sha.ComputeHash(ms);
                    var hex = new StringBuilder(digest.Length * 2);
                    foreach (var b in digest)
                        hex.Append(b.ToString("x2", CultureInfo.InvariantCulture));
                    return hex.ToString();
                }
            }
        }

        private static void WriteU32(Stream s, uint value)
        {
            s.WriteByte((byte)(value & 0xff));
            s.WriteByte((byte)((value >> 8) & 0xff));
            s.WriteByte((byte)((value >> 16) & 0xff));
            s.WriteByte((byte)((value >> 24) & 0xff));
        }

        private static void WriteI64(Stream s, long value)
        {
            var bytes = BitConverter.GetBytes(value); // little-endian on all supported platforms
            if (!BitConverter.IsLittleEndian)
                Array.Reverse(bytes);
            s.Write(bytes, 0, bytes.Length);
        }
    }
}
