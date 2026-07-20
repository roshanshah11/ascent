using System.Collections.Generic;
using Ascent.Runtime.Presentation;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Ascent.Editor
{
    /// <summary>
    /// Deterministically (re)builds the Black Brant IX review scene structure:
    /// a two-stage vehicle, five camera rigs, engineering-layer hosts, and a
    /// neutral environment. Runs from the menu or headlessly via
    /// <c>-executeMethod Ascent.Editor.BlackBrantSceneBuilder.BuildFromBatch</c>.
    ///
    /// This authors the *structure* only. HDRP materials, terrain, VFX plume,
    /// and Cinemachine rigs are layered on interactively; the review controller
    /// binds behavior to the anchors created here. Nothing here is a source of
    /// canonical flight state — that comes only from the accepted FlightTrace.
    /// </summary>
    public static class BlackBrantSceneBuilder
    {
        public const string ScenePath = "Assets/Ascent/Scenes/BlackBrantIX.unity";
        public const string VehicleRootName = "BlackBrantIX_Vehicle";
        public const string BoosterName = "Stage_Booster_Terrier";
        public const string SustainerName = "Stage_Sustainer_BlackBrant";
        public const string CameraRigName = "CameraRig";
        public const string LayersRootName = "EngineeringLayers";

        [MenuItem("Ascent/Build Black Brant IX Scene")]
        public static void BuildFromMenu()
        {
            var scene = Build();
            EditorSceneManager.SaveScene(scene, ScenePath);
        }

        /// <summary>Batchmode entry point; saves and exits with a clear code.</summary>
        public static void BuildFromBatch()
        {
            try
            {
                var scene = Build();
                System.IO.Directory.CreateDirectory("Assets/Ascent/Scenes");
                bool ok = EditorSceneManager.SaveScene(scene, ScenePath);
                if (!ok)
                {
                    Debug.LogError("BlackBrantSceneBuilder: SaveScene failed");
                    EditorApplication.Exit(3);
                    return;
                }
                AssetDatabase.SaveAssets();
                Debug.Log($"BlackBrantSceneBuilder: wrote {ScenePath}");
                EditorApplication.Exit(0);
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"BlackBrantSceneBuilder failed: {ex}");
                EditorApplication.Exit(4);
            }
        }

        public static Scene Build()
        {
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);

            var anchorsGo = new GameObject("SceneAnchors");
            var anchors = anchorsGo.AddComponent<SceneAnchors>();

            // --- Vehicle: two separable stages (structural primitives) ---
            var vehicle = new GameObject(VehicleRootName).transform;
            anchors.vehicleRoot = vehicle;

            var booster = MakeStage(BoosterName, vehicle, yOffset: 0.0f, height: 5.8f, radius: 0.33f);
            var sustainer = MakeStage(SustainerName, vehicle, yOffset: 5.8f, height: 5.5f, radius: 0.22f);
            anchors.boosterStage = booster;
            anchors.sustainerStage = sustainer;

            // --- Five camera rigs, one per CameraDirector id ---
            var rig = new GameObject(CameraRigName).transform;
            anchors.cameraRig = rig;
            foreach (var id in CameraDirector.Ids)
            {
                var camGo = new GameObject($"Camera_{id}");
                camGo.transform.SetParent(rig, false);
                var cam = camGo.AddComponent<Camera>();
                cam.enabled = id == CameraDirector.Pad; // pad establishes scale first
                PlaceCamera(id, camGo.transform);
                anchors.cameras.Add(new SceneAnchors.NamedTransform { id = id, transform = camGo.transform });
            }

            // --- Engineering-layer host objects, one per always-on layer ---
            var layersRoot = new GameObject(LayersRootName).transform;
            anchors.engineeringLayers = layersRoot;
            foreach (var layer in EngineeringLayerCatalog.Always())
            {
                var host = new GameObject($"Layer_{layer.Id}").transform;
                host.SetParent(layersRoot, false);
                anchors.layerHosts.Add(new SceneAnchors.NamedTransform { id = layer.Id, transform = host });
            }

            // --- Neutral environment (HDRP volume/terrain added interactively) ---
            var ground = GameObject.CreatePrimitive(PrimitiveType.Plane);
            ground.name = "Ground";
            ground.transform.localScale = new Vector3(200f, 1f, 200f);

            var sunGo = new GameObject("Sun");
            var sun = sunGo.AddComponent<Light>();
            sun.type = LightType.Directional;
            sun.intensity = 1.1f;
            sunGo.transform.rotation = Quaternion.Euler(50f, -30f, 0f);

            // --- Review shell host (UIDocument wired interactively) ---
            new GameObject("ReviewWorkbench");

            return scene;
        }

        private static Transform MakeStage(string name, Transform parent, float yOffset, float height, float radius)
        {
            var stage = new GameObject(name).transform;
            stage.SetParent(parent, false);

            var body = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            body.name = $"{name}_Body";
            body.transform.SetParent(stage, false);
            // Unity cylinder is 2 units tall by default; scale to height.
            body.transform.localScale = new Vector3(radius * 2f, height / 2f, radius * 2f);
            body.transform.localPosition = new Vector3(0f, yOffset + height / 2f, 0f);
            // Scientific motion: no collider-driven physics.
            var col = body.GetComponent<Collider>();
            if (col != null)
                Object.DestroyImmediate(col);

            return stage;
        }

        private static void PlaceCamera(string id, Transform t)
        {
            switch (id)
            {
                case CameraDirector.Pad:
                    t.localPosition = new Vector3(0f, 3f, -20f);
                    t.localRotation = Quaternion.Euler(5f, 0f, 0f);
                    break;
                case CameraDirector.Chase:
                    t.localPosition = new Vector3(0f, 8f, -30f);
                    break;
                case CameraDirector.Onboard:
                    t.localPosition = new Vector3(0f, 11f, 0.5f);
                    t.localRotation = Quaternion.Euler(80f, 0f, 0f);
                    break;
                case CameraDirector.GroundTracking:
                    t.localPosition = new Vector3(60f, 2f, -10f);
                    t.localRotation = Quaternion.Euler(0f, -70f, 0f);
                    break;
                case CameraDirector.Inspection:
                    t.localPosition = new Vector3(6f, 6f, -6f);
                    t.localRotation = Quaternion.Euler(20f, -45f, 0f);
                    break;
            }
        }
    }
}
