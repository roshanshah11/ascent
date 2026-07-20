using System;
using System.Collections.Generic;
using System.Text;
using Newtonsoft.Json;

namespace Ascent.Runtime.Bridge
{
    /// <summary>Raised on any framing, encoding, or protocol-version violation.</summary>
    public sealed class FrameException : Exception
    {
        public FrameException(string message) : base(message) { }
    }

    /// <summary>
    /// Reassembles a byte stream into whole <see cref="Envelope"/> frames. Mirrors
    /// the Rust <c>FrameDecoder</c>: four-byte little-endian length + UTF-8 JSON,
    /// bounded by a maximum, fail-closed. Any violation poisons the decoder
    /// permanently.
    /// </summary>
    public sealed class FrameDecoder
    {
        private readonly int _maxFrameBytes;
        private readonly List<byte> _buffer = new List<byte>();
        private bool _poisoned;

        public FrameDecoder(int maxFrameBytes)
        {
            _maxFrameBytes = maxFrameBytes;
        }

        public bool IsPoisoned => _poisoned;

        /// <summary>Encode a payload token as one length-prefixed frame.</summary>
        public static byte[] Encode(object envelope)
        {
            var json = JsonConvert.SerializeObject(envelope);
            var body = Encoding.UTF8.GetBytes(json);
            if (body.Length > Protocol.MaxFrameBytes)
                throw new FrameException("frame exceeds maximum size");
            var frame = new byte[4 + body.Length];
            frame[0] = (byte)(body.Length & 0xff);
            frame[1] = (byte)((body.Length >> 8) & 0xff);
            frame[2] = (byte)((body.Length >> 16) & 0xff);
            frame[3] = (byte)((body.Length >> 24) & 0xff);
            Array.Copy(body, 0, frame, 4, body.Length);
            return frame;
        }

        public IReadOnlyList<Envelope> Push(ReadOnlySpan<byte> bytes)
        {
            if (_poisoned)
                throw new FrameException("decoder is poisoned");

            foreach (var b in bytes)
                _buffer.Add(b);

            var outFrames = new List<Envelope>();
            while (true)
            {
                if (_buffer.Count < 4)
                    break;
                long len = (uint)(_buffer[0]
                    | (_buffer[1] << 8)
                    | (_buffer[2] << 16)
                    | (_buffer[3] << 24));
                if (len == 0)
                    throw Poison("zero-length frame");
                if (len > _maxFrameBytes)
                    throw Poison($"frame length {len} exceeds maximum {_maxFrameBytes}");
                if (_buffer.Count < 4 + len)
                    break;

                var body = new byte[len];
                _buffer.CopyTo(4, body, 0, (int)len);
                _buffer.RemoveRange(0, 4 + (int)len);

                string text;
                try
                {
                    text = new UTF8Encoding(false, true).GetString(body);
                }
                catch (Exception)
                {
                    throw Poison("frame payload is not valid UTF-8");
                }

                Envelope envelope;
                try
                {
                    envelope = JsonConvert.DeserializeObject<Envelope>(text);
                }
                catch (Exception)
                {
                    throw Poison("frame payload is not valid JSON");
                }
                if (envelope == null)
                    throw Poison("frame payload is not valid JSON");
                if (envelope.ProtocolVersion != Protocol.Version)
                    throw Poison($"protocol version {envelope.ProtocolVersion} is not supported");

                outFrames.Add(envelope);
            }
            return outFrames;
        }

        private FrameException Poison(string message)
        {
            _poisoned = true;
            return new FrameException(message);
        }
    }
}
