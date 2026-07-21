using System.Reflection;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.HighDefinition;

namespace Ascent.Editor
{
    /// <summary>
    /// Converts the project to the High Definition Render Pipeline: creates an
    /// <see cref="HDRenderPipelineAsset"/> if one is not already assigned, wires
    /// it into <see cref="GraphicsSettings"/> and every quality level, and
    /// ensures HDRP global settings exist. Idempotent — safe to re-run.
    ///
    /// Runs from the menu or headlessly via
    /// <c>-executeMethod Ascent.Editor.HdrpSetup.ApplyFromBatch</c>.
    /// </summary>
    public static class HdrpSetup
    {
        public const string AssetPath = "Assets/Ascent/Rendering/AscentHDRP.asset";

        [MenuItem("Ascent/Set Up HDRP")]
        public static void ApplyFromMenu() => Apply();

        public static void ApplyFromBatch()
        {
            try
            {
                Apply();
                AssetDatabase.SaveAssets();
                Debug.Log("HdrpSetup: HDRP pipeline asset assigned");
                EditorApplication.Exit(0);
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"HdrpSetup failed: {ex}");
                EditorApplication.Exit(5);
            }
        }

        public static HDRenderPipelineAsset Apply()
        {
            // Global settings must exist before the pipeline asset is used, or
            // HDRP logs default-resource errors. Ensure() is internal, so reach
            // it reflectively; failure is non-fatal (Unity self-heals on render).
            EnsureGlobalSettings();

            var asset = GraphicsSettings.defaultRenderPipeline as HDRenderPipelineAsset;
            if (asset == null)
            {
                System.IO.Directory.CreateDirectory("Assets/Ascent/Rendering");
                asset = ScriptableObject.CreateInstance<HDRenderPipelineAsset>();
                AssetDatabase.CreateAsset(asset, AssetPath);
            }

            GraphicsSettings.defaultRenderPipeline = asset;
            // Pin every quality level to the same asset so headless/nographics
            // runs never fall back to the built-in pipeline mid-session.
            int previous = QualitySettings.GetQualityLevel();
            var names = QualitySettings.names;
            for (int i = 0; i < names.Length; i++)
            {
                QualitySettings.SetQualityLevel(i, false);
                QualitySettings.renderPipeline = asset;
            }
            QualitySettings.SetQualityLevel(previous, false);

            EditorUtility.SetDirty(asset);
            return asset;
        }

        private static void EnsureGlobalSettings()
        {
            try
            {
                // The global-settings type is internal to the HDRP runtime, so it
                // cannot be named at compile time — resolve it by string instead.
                var t = System.Type.GetType(
                    "UnityEngine.Rendering.HighDefinition.HDRenderPipelineGlobalSettings, "
                    + "Unity.RenderPipelines.HighDefinition.Runtime");
                var ensure = t?.GetMethod(
                    "Ensure",
                    BindingFlags.Static | BindingFlags.NonPublic | BindingFlags.Public);
                ensure?.Invoke(null, new object[] { true });
            }
            catch (System.Exception ex)
            {
                Debug.LogWarning($"HdrpSetup: could not pre-create global settings ({ex.Message}); "
                    + "HDRP will create them on first render.");
            }
        }
    }
}
