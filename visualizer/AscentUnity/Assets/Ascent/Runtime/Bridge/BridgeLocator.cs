using System;
using System.Diagnostics;
using System.IO;
using UnityEngine;

namespace Ascent.Runtime.Bridge
{
    /// <summary>
    /// Resolves the read-only bridge executable for the current run target and
    /// makes it launchable. A packaged player runs the copy staged under
    /// StreamingAssets (placed there by <c>PlayerBuilder</c>); the editor and any
    /// dev run use the Cargo debug build at the repository root. On macOS/Linux the
    /// executable bit is restored on first launch, since asset copying can drop it.
    /// This is the single place that knows where the bridge lives.
    /// </summary>
    public static class BridgeLocator
    {
        public const string ExecutableName = "ascent-visualizer-bridge";

        /// <summary>The path the bridge should launch from, or null if none exists.</summary>
        public static string Resolve()
        {
            // Packaged player: staged beside the app under StreamingAssets.
            var staged = Path.Combine(Application.streamingAssetsPath, ExecutableName);
            if (File.Exists(staged))
                return staged;

            // Editor / dev: the Cargo debug build at <repo>/target/debug.
            // Application.dataPath = <repo>/visualizer/AscentUnity/Assets
            var dev = Path.GetFullPath(Path.Combine(
                Application.dataPath, "../../../target/debug/" + ExecutableName));
            if (File.Exists(dev))
                return dev;

            return null;
        }

        /// <summary>Best-effort restore of the executable bit (no-op on Windows).</summary>
        public static void EnsureExecutable(string path)
        {
            if (string.IsNullOrEmpty(path) || !File.Exists(path))
                return;
            if (Application.platform == RuntimePlatform.WindowsPlayer ||
                Application.platform == RuntimePlatform.WindowsEditor)
                return;
            try
            {
                using var chmod = Process.Start(new ProcessStartInfo
                {
                    FileName = "/bin/chmod",
                    Arguments = "+x \"" + path + "\"",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                });
                chmod?.WaitForExit(2000);
            }
            catch (Exception)
            {
                // If chmod is unavailable the file may already be executable; the
                // launch attempt will surface any genuine permission failure.
            }
        }
    }
}
