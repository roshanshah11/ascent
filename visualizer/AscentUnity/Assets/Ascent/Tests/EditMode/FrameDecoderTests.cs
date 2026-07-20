using System;
using System.Linq;
using Ascent.Runtime.Bridge;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    public class FrameDecoderTests
    {
        [Test]
        public void DecoderRetainsPartialPrefixAndPayload()
        {
            var bytes = Fixtures.ReadFrame("server/hello_ack.frame");
            var decoder = new FrameDecoder(16 * 1024 * 1024);
            Assert.That(decoder.Push(new ReadOnlySpan<byte>(bytes, 0, 3)), Is.Empty);
            var decoded = decoder.Push(new ReadOnlySpan<byte>(bytes, 3, bytes.Length - 3));
            Assert.That(decoded.Single().Kind, Is.EqualTo("hello_ack"));
        }

        [Test]
        public void DecoderReassemblesByteAtATime()
        {
            var bytes = Fixtures.ReadFrame("client/hello.frame");
            var decoder = new FrameDecoder(16 * 1024 * 1024);
            for (int i = 0; i < bytes.Length; i++)
            {
                var decoded = decoder.Push(new ReadOnlySpan<byte>(bytes, i, 1));
                if (i + 1 == bytes.Length)
                    Assert.That(decoded.Single().Kind, Is.EqualTo("hello"));
                else
                    Assert.That(decoded, Is.Empty);
            }
        }

        [TestCase("server/mission_catalog.frame", "mission_catalog")]
        [TestCase("server/trace_manifest.frame", "trace_manifest")]
        [TestCase("server/trace_channel_chunk.frame", "trace_channel_chunk")]
        [TestCase("server/trace_events.frame", "trace_events")]
        [TestCase("server/run_completed.frame", "run_completed")]
        [TestCase("server/run_cancelled.frame", "run_cancelled")]
        [TestCase("server/error.frame", "error")]
        [TestCase("client/run_mission.frame", "run_mission")]
        public void ServerAndClientFixturesDecodeToDeclaredKind(string rel, string kind)
        {
            var bytes = Fixtures.ReadFrame(rel);
            var decoder = new FrameDecoder(16 * 1024 * 1024);
            Assert.That(decoder.Push(bytes).Single().Kind, Is.EqualTo(kind));
        }

        [TestCase("malformed/zero_length.frame")]
        [TestCase("malformed/oversize_header.frame")]
        [TestCase("malformed/bad_json.frame")]
        [TestCase("malformed/bad_utf8.frame")]
        [TestCase("malformed/wrong_version.frame")]
        public void CodecLevelMalformedFramesFailClosed(string rel)
        {
            var bytes = Fixtures.ReadFrame(rel);
            var decoder = new FrameDecoder(16 * 1024 * 1024);
            Assert.Throws<FrameException>(() => decoder.Push(bytes));
            Assert.Throws<FrameException>(() => decoder.Push(new byte[] { 1 }));
        }
    }
}
