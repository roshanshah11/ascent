using System.Collections.Generic;
using System.Linq;
using Ascent.Runtime.Playback;
using Ascent.Runtime.Trace;

namespace Ascent.Runtime.Presentation
{
    /// <summary>An immutable snapshot of everything the review shell is showing.</summary>
    public readonly struct ReviewState
    {
        public readonly double TimeSeconds;
        public readonly string CameraId;
        public readonly double Rate;
        public readonly bool IsPlaying;
        public readonly bool CleanView;
        public readonly IReadOnlyList<string> VisibleLayers;

        public ReviewState(double time, string cameraId, double rate, bool isPlaying, bool cleanView, IReadOnlyList<string> visibleLayers)
        {
            TimeSeconds = time;
            CameraId = cameraId;
            Rate = rate;
            IsPlaying = isPlaying;
            CleanView = cleanView;
            VisibleLayers = visibleLayers;
        }

        /// <summary>Same review state with layer visibility (and clean-view flag) dropped,
        /// so two states can be compared for "did anything but layers change?".</summary>
        public ReviewState WithoutLayerVisibility() =>
            new ReviewState(TimeSeconds, CameraId, Rate, IsPlaying, false, new string[0]);

        public bool EqualsIgnoringLayers(ReviewState other)
        {
            return TimeSeconds == other.TimeSeconds
                && CameraId == other.CameraId
                && Rate == other.Rate
                && IsPlaying == other.IsPlaying;
        }
    }

    /// <summary>
    /// Headless-testable review controller: owns the playback clock, camera
    /// selection, and engineering-layer visibility over an accepted trace. Clean
    /// view hides every layer without touching time, camera, rate, or play state,
    /// and remembers the prior set so it can be restored.
    /// </summary>
    public sealed class ReviewWorkbench
    {
        private readonly FlightTrace _trace;
        private readonly PlaybackClock _clock;
        private readonly CameraDirector _cameras = new CameraDirector();
        private readonly HashSet<string> _visible = new HashSet<string>();
        private string[] _restoreAfterClean;
        private bool _cleanView;

        public ReviewWorkbench(FlightTrace trace, double frameRate)
        {
            _trace = trace;
            _clock = new PlaybackClock(frameRate, trace.StartSeconds, trace.EndSeconds);
            foreach (var layer in EngineeringLayerCatalog.Always())
                _visible.Add(layer.Id);
        }

        public PlaybackClock Clock => _clock;
        public CameraDirector Cameras => _cameras;
        public FlightTrace Trace => _trace;
        public int VisibleEngineeringLayerCount => _visible.Count;
        public bool CleanView => _cleanView;

        public void ToggleLayer(string id)
        {
            if (!_visible.Remove(id))
                _visible.Add(id);
        }

        public void SetCleanView(bool on)
        {
            if (on == _cleanView)
                return;
            if (on)
            {
                _restoreAfterClean = _visible.ToArray();
                _visible.Clear();
            }
            else if (_restoreAfterClean != null)
            {
                _visible.Clear();
                foreach (var id in _restoreAfterClean)
                    _visible.Add(id);
            }
            _cleanView = on;
        }

        /// <summary>Seek exactly to a named event's Rust time; false if absent.</summary>
        public bool SeekEvent(string kind)
        {
            var t = _trace.SeekEvent(kind);
            if (t == null)
                return false;
            _clock.Seek(t.Value);
            return true;
        }

        public ReviewState Snapshot() => new ReviewState(
            _clock.TimeSeconds,
            _cameras.ActiveId,
            _clock.Rate,
            _clock.IsPlaying,
            _cleanView,
            _visible.OrderBy(x => x).ToArray());
    }

    /// <summary>The fixed keyboard contract for the review shell.</summary>
    public static class ReviewKeys
    {
        public const string PlayPause = "Space";
        public const string FrameStepBack = "Left";
        public const string FrameStepForward = "Right";
        public const string EventJumpBack = "Shift+Left";
        public const string EventJumpForward = "Shift+Right";
        public const string CameraDigits = "1-5";
        public const string ToggleLayers = "L";
        public const string CleanView = "C";
    }
}
