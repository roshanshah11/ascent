using System.Collections.Generic;
using Ascent.Runtime.Presentation;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.SceneManagement;
using UnityEngine.UIElements;

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
                RegisterInBuildSettings(ScenePath);
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

            // --- Trace-driven exhaust plume at the nozzle (base of the stack) ---
            var plumeGo = BuildPlume(vehicle);
            anchors.plume = plumeGo.transform;

            // --- One rendering camera with a Cinemachine brain + five vcams ---
            var reviewCam = new GameObject("ReviewCamera");
            reviewCam.AddComponent<Camera>();
            reviewCam.AddComponent<Unity.Cinemachine.CinemachineBrain>();
            anchors.reviewCamera = reviewCam.transform;

            var rig = new GameObject(CameraRigName).transform;
            anchors.cameraRig = rig;
            var cameraRigComp = rig.gameObject.AddComponent<CameraRig>();
            foreach (var id in CameraDirector.Ids)
            {
                var camGo = new GameObject($"Vcam_{id}");
                camGo.transform.SetParent(rig, false);
                var vcam = camGo.AddComponent<Unity.Cinemachine.CinemachineCamera>();
                // Pad establishes scale first; it starts as the live shot.
                vcam.Priority = id == CameraDirector.Pad ? CameraRig.ActivePriority : CameraRig.IdlePriority;
                // Tracking shots follow the vehicle; pad and inspection are static.
                if (id == CameraDirector.Chase || id == CameraDirector.Onboard || id == CameraDirector.GroundTracking)
                {
                    vcam.Follow = vehicle;
                    vcam.LookAt = vehicle;
                }
                PlaceCamera(id, camGo.transform);
                anchors.cameras.Add(new SceneAnchors.NamedTransform { id = id, transform = camGo.transform });
                cameraRigComp.vcams.Add(new SceneAnchors.NamedTransform { id = id, transform = camGo.transform });
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

            // --- HDRP environment: White Sands terrain, directional sun, volume ---
            anchors.terrain = BuildTerrain().transform;

            var sunGo = new GameObject("Sun");
            var sun = sunGo.AddComponent<Light>();
            sun.type = LightType.Directional;
            sun.intensity = 1.1f;
            sunGo.transform.rotation = Quaternion.Euler(50f, -30f, 0f);

            // Global volume: an empty profile lets the HDRP pipeline defaults
            // (sky, exposure, tonemapping) drive the look; overrides are authored
            // interactively on top of this anchor.
            var volumeGo = new GameObject("GlobalVolume");
            var volume = volumeGo.AddComponent<Volume>();
            volume.isGlobal = true;
            volume.priority = 0f;
            var profile = ScriptableObject.CreateInstance<VolumeProfile>();
            System.IO.Directory.CreateDirectory("Assets/Ascent/Rendering");
            AssetDatabase.CreateAsset(profile, "Assets/Ascent/Rendering/GlobalVolume.asset");
            volume.sharedProfile = profile;

            // --- Review shell: live UIDocument mounting the authored workbench ---
            var workbenchGo = new GameObject("ReviewWorkbench");
            var uiDoc = workbenchGo.AddComponent<UIDocument>();
            uiDoc.panelSettings = EnsurePanelSettings();
            uiDoc.visualTreeAsset =
                AssetDatabase.LoadAssetAtPath<VisualTreeAsset>("Assets/Ascent/UI/ReviewWorkbench.uxml");
            if (uiDoc.visualTreeAsset == null)
                Debug.LogWarning("BlackBrantSceneBuilder: ReviewWorkbench.uxml not found; HUD will be empty");
            var hud = workbenchGo.AddComponent<ReviewHud>();
            // Inert unless a standalone player is launched with -ascent-benchmark;
            // records packaged FPS for the platform-decision performance gate.
            workbenchGo.AddComponent<PackagedBenchmark>();

            // The runtime driver: launches the bridge, runs the mission to an
            // accepted trace, binds the HUD + playback, and drives the vehicle and
            // plume from the trace. Without it the scene shows only an idle shell.
            var bootstrap = workbenchGo.AddComponent<ReviewBootstrap>();
            bootstrap.anchors = anchors;
            bootstrap.hud = hud;
            bootstrap.plume = plumeGo.GetComponent<PlumeController>();

            anchors.reviewShell = workbenchGo.transform;

            return scene;
        }

        /// <summary>
        /// Ensures the review scene is registered (and enabled) in the editor build
        /// settings so a runtime <c>SceneManager.LoadScene</c> can find it by name —
        /// needed by the PlayMode performance gate and any packaged build.
        /// </summary>
        private static void RegisterInBuildSettings(string path)
        {
            foreach (var s in EditorBuildSettings.scenes)
                if (s.path == path)
                    return;
            var list = new List<EditorBuildSettingsScene>(EditorBuildSettings.scenes)
            {
                new EditorBuildSettingsScene(path, true),
            };
            EditorBuildSettings.scenes = list.ToArray();
            Debug.Log($"BlackBrantSceneBuilder: registered {path} in build settings");
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

            // Metallic airframe tone so the vehicle reads under HDRP lighting.
            AssignHdrpMaterial(body, $"{name}_Mat", new Color(0.55f, 0.57f, 0.60f), metallic: 0.85f, smoothness: 0.55f);

            return stage;
        }

        /// <summary>
        /// Builds the exhaust plume: a downward cone <see cref="ParticleSystem"/>
        /// at the nozzle, driven at runtime by <see cref="PlumeController"/> from
        /// the trace's powered intervals. Emission starts off; the controller
        /// turns it on during burns.
        /// </summary>
        private static GameObject BuildPlume(Transform vehicle)
        {
            var go = new GameObject("ExhaustPlume");
            go.transform.SetParent(vehicle, false);
            go.transform.localPosition = new Vector3(0f, -0.2f, 0f); // just below the nozzle
            go.transform.localRotation = Quaternion.Euler(90f, 0f, 0f); // emit downward (-Y)

            var ps = go.AddComponent<ParticleSystem>();
            var main = ps.main;
            main.startLifetime = 0.6f;
            main.startSpeed = 0f; // controller sets speed when powered
            main.startSize = 0.6f;
            main.startColor = new Color(1.0f, 0.75f, 0.4f, 0.9f); // hot exhaust
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.maxParticles = 2000;

            var emission = ps.emission;
            emission.rateOverTime = 0f; // off until the controller detects a burn

            var shape = ps.shape;
            shape.shapeType = ParticleSystemShapeType.Cone;
            shape.angle = 12f;
            shape.radius = 0.2f;

            // Stop auto-play; the controller drives Play/Stop from trace state.
            var emissionModule = ps.emission;
            emissionModule.enabled = true;
            ps.Stop(true, ParticleSystemStopBehavior.StopEmitting);

            go.AddComponent<PlumeController>();
            return go;
        }

        /// <summary>
        /// Builds the White Sands gypsum terrain the pad sits on: a deterministic
        /// dune heightfield that stays flat around the origin (so the vehicle rests
        /// at ground level) and rises toward the edges. The gypsum tone comes from a
        /// solid TerrainLayer; the HDRP terrain material is applied when available.
        /// Terrain creation always yields a valid ground even if a shader is missing.
        /// </summary>
        private static Transform BuildTerrain()
        {
            const int res = 129;                 // heightmap resolution (2^n + 1)
            const float extent = 600f;           // terrain span in metres
            const float maxHeight = 18f;         // dune amplitude
            const float padRadius = 60f;         // flat launch apron radius (metres)

            var data = new TerrainData
            {
                heightmapResolution = res,
                size = new Vector3(extent, maxHeight, extent),
            };

            var heights = new float[res, res];
            float half = extent / 2f;
            for (int y = 0; y < res; y++)
            {
                for (int x = 0; x < res; x++)
                {
                    // World offset from the terrain centre (origin sits at centre).
                    float wx = (x / (float)(res - 1)) * extent - half;
                    float wz = (y / (float)(res - 1)) * extent - half;
                    // Deterministic crossed sine dunes (no RNG — repeatable builds).
                    float dune = 0.5f
                        + 0.28f * Mathf.Sin(wx * 0.018f)
                        + 0.18f * Mathf.Sin(wz * 0.026f + 1.3f)
                        + 0.10f * Mathf.Sin((wx + wz) * 0.011f);
                    // Flatten to zero within the pad apron, easing out to full dunes.
                    float d = Mathf.Sqrt(wx * wx + wz * wz);
                    float apron = Mathf.Clamp01((d - padRadius) / padRadius);
                    heights[y, x] = Mathf.Clamp01(dune) * apron;
                }
            }
            data.SetHeights(0, 0, heights);

            System.IO.Directory.CreateDirectory("Assets/Ascent/Rendering");
            var gypsum = MakeSolidTexture(new Color(0.87f, 0.84f, 0.77f), "WhiteSandsGypsum");
            var layer = new TerrainLayer { diffuseTexture = gypsum, tileSize = new Vector2(30f, 30f) };
            AssetDatabase.CreateAsset(layer, "Assets/Ascent/Rendering/WhiteSandsLayer.terrainlayer");
            data.terrainLayers = new[] { layer };
            AssetDatabase.CreateAsset(data, "Assets/Ascent/Rendering/WhiteSandsTerrain.asset");

            var go = Terrain.CreateTerrainGameObject(data);
            go.name = "WhiteSandsTerrain";
            go.transform.position = new Vector3(-half, 0f, -half); // centre the flat apron on origin

            var terrain = go.GetComponent<Terrain>();
            var terrainShader = Shader.Find("HDRP/TerrainLit");
            if (terrain != null && terrainShader != null)
                terrain.materialTemplate = new Material(terrainShader) { name = "WhiteSandsTerrainMat" };

            return go.transform;
        }

        /// <summary>Creates and saves a small solid-colour texture asset.</summary>
        private static Texture2D MakeSolidTexture(Color color, string assetName)
        {
            var tex = new Texture2D(4, 4, TextureFormat.RGBA32, false) { name = assetName };
            var pixels = new Color[16];
            for (int i = 0; i < pixels.Length; i++)
                pixels[i] = color;
            tex.SetPixels(pixels);
            tex.Apply();
            AssetDatabase.CreateAsset(tex, $"Assets/Ascent/Rendering/{assetName}.asset");
            return tex;
        }

        /// <summary>
        /// Assigns a fresh HDRP/Lit material to <paramref name="go"/>. Falls back
        /// silently to whatever shader the renderer already has if the HDRP shader
        /// cannot be resolved (e.g. a built-in fallback session).
        /// </summary>
        private static void AssignHdrpMaterial(
            GameObject go, string assetName, Color baseColor, float metallic = 0f, float smoothness = 0.4f)
        {
            var shader = Shader.Find("HDRP/Lit");
            if (shader == null)
                return;
            var mat = new Material(shader) { name = assetName };
            mat.SetColor("_BaseColor", baseColor);
            if (mat.HasProperty("_Metallic"))
                mat.SetFloat("_Metallic", metallic);
            if (mat.HasProperty("_Smoothness"))
                mat.SetFloat("_Smoothness", smoothness);
            System.IO.Directory.CreateDirectory("Assets/Ascent/Rendering/Materials");
            AssetDatabase.CreateAsset(mat, $"Assets/Ascent/Rendering/Materials/{assetName}.mat");
            var renderer = go.GetComponent<Renderer>();
            if (renderer != null)
                renderer.sharedMaterial = mat;
        }

        /// <summary>
        /// Loads (or creates) the shared <see cref="PanelSettings"/> for the review
        /// HUD, giving it a best-effort default runtime theme. A missing theme only
        /// leaves the panel unstyled — it still mounts the UXML — so theme failure
        /// never breaks the build; a designer can assign one interactively later.
        /// </summary>
        private static PanelSettings EnsurePanelSettings()
        {
            const string path = "Assets/Ascent/UI/AscentPanelSettings.asset";
            var existing = AssetDatabase.LoadAssetAtPath<PanelSettings>(path);
            if (existing != null)
                return existing;

            var settings = ScriptableObject.CreateInstance<PanelSettings>();
            var theme = EnsureRuntimeTheme();
            if (theme != null)
                settings.themeStyleSheet = theme;
            System.IO.Directory.CreateDirectory("Assets/Ascent/UI");
            AssetDatabase.CreateAsset(settings, path);
            return settings;
        }

        /// <summary>
        /// Best-effort load/creation of the default runtime theme. Returns null on
        /// any failure; callers treat a null theme as "render unstyled".
        /// </summary>
        private static ThemeStyleSheet EnsureRuntimeTheme()
        {
            const string themePath = "Assets/Ascent/UI/AscentRuntimeTheme.tss";
            var existing = AssetDatabase.LoadAssetAtPath<ThemeStyleSheet>(themePath);
            if (existing != null)
                return existing;
            try
            {
                if (!System.IO.File.Exists(themePath))
                {
                    // The canonical default runtime theme is a single engine import.
                    System.IO.File.WriteAllText(themePath, "@import url(\"unity-theme://default\");\n");
                    AssetDatabase.ImportAsset(themePath, ImportAssetOptions.ForceSynchronousImport);
                }
                return AssetDatabase.LoadAssetAtPath<ThemeStyleSheet>(themePath);
            }
            catch (System.Exception ex)
            {
                Debug.LogWarning($"BlackBrantSceneBuilder: could not create runtime theme ({ex.Message}); HUD renders unstyled");
                return null;
            }
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
