using System;
using Ascent.Runtime.Playback;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    public class PlaybackClockTests
    {
        [Test]
        public void AdvanceScalesByRateAndStopsAtRangeEnd()
        {
            var clock = new PlaybackClock(frameRate: 60.0, rangeStart: 0.0, rangeEnd: 10.0);
            clock.SetRate(2.0);
            clock.Play();
            clock.Advance(1.0);
            Assert.That(clock.TimeSeconds, Is.EqualTo(2.0).Within(1e-9));
            clock.Advance(100.0);
            Assert.That(clock.TimeSeconds, Is.EqualTo(10.0).Within(1e-9));
            Assert.That(clock.IsPlaying, Is.False);
        }

        [Test]
        public void SeekAndStepClampToRangeAndPause()
        {
            var clock = new PlaybackClock(60.0, 1.0, 5.0);
            clock.Seek(-3.0);
            Assert.That(clock.TimeSeconds, Is.EqualTo(1.0));
            clock.Seek(99.0);
            Assert.That(clock.TimeSeconds, Is.EqualTo(5.0));
            clock.Play();
            clock.StepFrames(-6); // 6 frames at 60fps = 0.1s back
            Assert.That(clock.IsPlaying, Is.False);
            Assert.That(clock.TimeSeconds, Is.EqualTo(4.9).Within(1e-9));
        }

        [Test]
        public void OnlyTheFixedRateSetIsAccepted()
        {
            var clock = new PlaybackClock(60.0, 0.0, 1.0);
            foreach (var r in PlaybackClock.Rates)
                Assert.DoesNotThrow(() => clock.SetRate(r));
            Assert.Throws<ArgumentException>(() => clock.SetRate(3.0));
        }
    }
}
