using System;
using System.Collections.Generic;
using System.Linq;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// The five-camera cinematic grammar. A camera change is presentation-only:
    /// this director holds no clock and returns no time, so switching can never
    /// alter playback. Actual Cinemachine blends are wired in the scene (Step 4
    /// visual authoring); this owns the selection contract and ordering.
    /// </summary>
    public sealed class CameraDirector
    {
        public const string Pad = "pad";
        public const string Chase = "chase";
        public const string Onboard = "onboard";
        public const string GroundTracking = "ground_tracking";
        public const string Inspection = "inspection";

        public static readonly IReadOnlyList<string> Ids = new[]
        {
            Pad, Chase, Onboard, GroundTracking, Inspection,
        };

        public string ActiveId { get; private set; } = Pad;

        public event Action<string> ActiveChanged;

        public void SwitchTo(string id)
        {
            if (!Ids.Contains(id))
                throw new ArgumentException($"unknown camera id '{id}'");
            if (ActiveId == id)
                return;
            ActiveId = id;
            ActiveChanged?.Invoke(id);
        }

        /// <summary>Map the digits 1–5 to camera ids for the keyboard contract.</summary>
        public static string ForDigit(int digit)
        {
            if (digit < 1 || digit > Ids.Count)
                throw new ArgumentOutOfRangeException(nameof(digit));
            return Ids[digit - 1];
        }
    }
}
