using System;

namespace Ascent.Runtime.Playback
{
    /// <summary>
    /// Presentation-only review clock over an accepted trace. It never reruns the
    /// Rust simulation; reverse and step are pure seeks. Rates are the fixed set
    /// {0.25, 0.5, 1, 2, 5}. The clock is clamped to its review range.
    /// </summary>
    public sealed class PlaybackClock
    {
        public static readonly double[] Rates = { 0.25, 0.5, 1.0, 2.0, 5.0 };

        private readonly double _frameRate;
        private double _rangeStart;
        private double _rangeEnd;
        private double _time;
        private double _rate = 1.0;

        public PlaybackClock(double frameRate, double rangeStart, double rangeEnd)
        {
            if (frameRate <= 0.0) throw new ArgumentOutOfRangeException(nameof(frameRate));
            _frameRate = frameRate;
            SetRange(rangeStart, rangeEnd);
            _time = _rangeStart;
        }

        public bool IsPlaying { get; private set; }
        public double TimeSeconds => _time;
        public double Rate => _rate;
        public double RangeStart => _rangeStart;
        public double RangeEnd => _rangeEnd;

        public void Play() => IsPlaying = true;
        public void Pause() => IsPlaying = false;

        public void Seek(double seconds)
        {
            _time = Clamp(seconds);
        }

        public void StepFrames(int frames)
        {
            Pause();
            _time = Clamp(_time + frames / _frameRate);
        }

        public void SetRate(double rate)
        {
            foreach (var r in Rates)
            {
                if (Math.Abs(r - rate) < 1e-9)
                {
                    _rate = r;
                    return;
                }
            }
            throw new ArgumentException($"unsupported rate {rate}; allowed: 0.25, 0.5, 1, 2, 5");
        }

        public void SetRange(double start, double end)
        {
            if (end < start) throw new ArgumentException("range end precedes start");
            _rangeStart = start;
            _rangeEnd = end;
            _time = Clamp(_time);
        }

        /// <summary>Advance by real seconds when playing; stops at the range end.</summary>
        public void Advance(double deltaSeconds)
        {
            if (!IsPlaying) return;
            _time += deltaSeconds * _rate;
            if (_time >= _rangeEnd)
            {
                _time = _rangeEnd;
                IsPlaying = false;
            }
            else if (_time < _rangeStart)
            {
                _time = _rangeStart;
            }
        }

        private double Clamp(double t)
        {
            if (t < _rangeStart) return _rangeStart;
            if (t > _rangeEnd) return _rangeEnd;
            return t;
        }
    }
}
