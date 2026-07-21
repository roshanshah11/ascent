using System.IO;
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
    }
}
