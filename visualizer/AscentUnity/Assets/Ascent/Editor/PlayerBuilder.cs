using System.IO;
using Ascent.Runtime.Bridge;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;

namespace Ascent.Editor
{
    /// <summary>
    /// Builds a standalone macOS development player containing the Black Brant IX
    /// review scene, headlessly via
    /// <c>-executeMethod Ascent.Editor.PlayerBuilder.BuildFromBatch</c>. The build
    /// is the packaged-smoke artifact; launched with <c>-ascent-benchmark</c> it
    /// records packaged FPS. Nothing is signed or distributed — this is a local
    /// development build for the platform-decision gate only.
    /// </summary>
    public static class PlayerBuilder
    {
        public const string OutputDir = "Build/StandaloneOSX";
        public const string AppName = "BlackBrantIX.app";

        public static void BuildFromBatch()
        {
            try
            {
                var scene = BlackBrantSceneBuilder.ScenePath;
                if (!File.Exists(scene))
                {
                    Debug.LogError($"PlayerBuilder: scene missing at {scene}; run BuildFromBatch first");
                    EditorApplication.Exit(3);
                    return;
                }

                StageBridgeBinary();

                Directory.CreateDirectory(OutputDir);
                var options = new BuildPlayerOptions
                {
                    scenes = new[] { scene },
                    locationPathName = Path.Combine(OutputDir, AppName),
                    target = BuildTarget.StandaloneOSX,
                    options = BuildOptions.Development,
                };

                BuildReport report = BuildPipeline.BuildPlayer(options);
                var summary = report.summary;
                Debug.Log($"PlayerBuilder: result={summary.result} size={summary.totalSize} " +
                          $"errors={summary.totalErrors} out={summary.outputPath}");

                EditorApplication.Exit(summary.result == BuildResult.Succeeded ? 0 : 5);
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"PlayerBuilder failed: {ex}");
                EditorApplication.Exit(6);
            }
        }

        /// <summary>
        /// Copies the Cargo-built read-only bridge into <c>Assets/StreamingAssets</c>
        /// so it ships inside the player (macOS: <c>.app/Contents/Resources/Data/
        /// StreamingAssets</c>). The runtime restores the executable bit on launch.
        /// Fails the build if the binary is absent — a player without it cannot run
        /// the review.
        /// </summary>
        private static void StageBridgeBinary()
        {
            // Application.dataPath = <repo>/visualizer/AscentUnity/Assets
            var src = Path.GetFullPath(Path.Combine(
                Application.dataPath, "../../../target/debug/" + BridgeLocator.ExecutableName));
            if (!File.Exists(src))
                throw new FileNotFoundException(
                    $"bridge binary missing at {src}; run `cargo build -p ascent-visualizer-bridge`");

            var dstDir = Path.Combine(Application.dataPath, "StreamingAssets");
            Directory.CreateDirectory(dstDir);
            var dst = Path.Combine(dstDir, BridgeLocator.ExecutableName);
            File.Copy(src, dst, overwrite: true);
            AssetDatabase.Refresh();
            Debug.Log($"PlayerBuilder: staged bridge → {dst}");
        }
    }
}
