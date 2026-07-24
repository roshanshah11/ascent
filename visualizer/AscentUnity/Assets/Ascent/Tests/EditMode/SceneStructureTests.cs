using System.Linq;
using Ascent.Runtime.Presentation;
using NUnit.Framework;
using Unity.Cinemachine;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.TestTools;

namespace Ascent.Tests.EditMode
{
    /// <summary>
    /// Structural smoke test for the built Black Brant IX scene: the two-stage
    /// vehicle, five camera rigs, engineering-layer hosts, and environment must
    /// all be present and wired to <see cref="SceneAnchors"/>. Verifies structure,
    /// not appearance — HDRP look and M3 performance are validated interactively.
    /// </summary>
    public class SceneStructureTests
    {
        private const string ScenePath = "Assets/Ascent/Scenes/BlackBrantIX.unity";

        private SceneAnchors OpenAndFindAnchors()
        {
            var scene = EditorSceneManager.OpenScene(ScenePath, OpenSceneMode.Single);
            Assert.That(scene.IsValid(), Is.True, "scene did not open");
            var anchors = scene.GetRootGameObjects()
                .Select(go => go.GetComponent<SceneAnchors>())
                .FirstOrDefault(a => a != null);
            Assert.That(anchors, Is.Not.Null, "SceneAnchors missing from scene");
            return anchors;
        }

        [Test]
        public void SceneHasTwoSeparableStages()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.vehicleRoot, Is.Not.Null);
            Assert.That(anchors.boosterStage, Is.Not.Null, "booster stage");
            Assert.That(anchors.sustainerStage, Is.Not.Null, "sustainer stage");
            // Stages are distinct children of the vehicle root. (Transform is
            // IEnumerable, so compare identity by instance id, not NUnit equality.)
            Assert.That(ReferenceEquals(anchors.boosterStage.parent, anchors.vehicleRoot), Is.True);
            Assert.That(ReferenceEquals(anchors.sustainerStage.parent, anchors.vehicleRoot), Is.True);
            Assert.That(anchors.boosterStage.GetInstanceID(),
                Is.Not.EqualTo(anchors.sustainerStage.GetInstanceID()));
        }

        [Test]
        public void SceneHasFiveVirtualCamerasOnePerDirectorId()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.cameras.Count, Is.EqualTo(5));
            foreach (var id in CameraDirector.Ids)
            {
                var cam = anchors.Camera(id);
                Assert.That(cam, Is.Not.Null, id);
                Assert.That(cam.GetComponent<Unity.Cinemachine.CinemachineCamera>(), Is.Not.Null,
                    $"{id} lacks a CinemachineCamera");
            }
        }

        [Test]
        public void SceneHasOneBrainCameraDrivingTheVcams()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.reviewCamera, Is.Not.Null, "review camera");
            Assert.That(anchors.reviewCamera.GetComponent<Camera>(), Is.Not.Null, "review camera lacks a Camera");
            Assert.That(anchors.reviewCamera.GetComponent<Unity.Cinemachine.CinemachineBrain>(), Is.Not.Null,
                "review camera lacks a CinemachineBrain");
        }

        [Test]
        public void TrackingCameraPresetsSnapAfterTimelineSeek()
        {
            var anchors = OpenAndFindAnchors();

            foreach (var id in CameraDirector.Ids)
            {
                var composer = anchors.Camera(id).GetComponent<CinemachineRotationComposer>();
                Assert.That(composer, Is.Not.Null, $"{id} must aim at its shot target");
                Assert.That(composer.Damping, Is.EqualTo(Vector2.zero),
                    $"{id} must reframe immediately after a timeline seek");
            }

            foreach (var id in new[] { CameraDirector.Chase, CameraDirector.Inspection })
            {
                var follow = anchors.Camera(id).GetComponent<CinemachineFollow>();
                Assert.That(follow, Is.Not.Null, $"{id} must follow the vehicle");
                Assert.That(follow.TrackerSettings.PositionDamping, Is.EqualTo(Vector3.zero),
                    $"{id} must not trail a timeline seek");
            }

            var onboard = anchors.Camera(CameraDirector.Onboard);
            Assert.That(onboard.GetComponent<CinemachineHardLockToTarget>(), Is.Not.Null,
                "onboard must be mounted on the vehicle");
            Assert.That(onboard.GetComponent<CinemachineHardLockToTarget>().Damping, Is.Zero,
                "onboard must not trail a timeline seek");
            Assert.That(onboard.GetComponent<CinemachineCamera>().Follow,
                Is.Not.EqualTo(anchors.vehicleRoot), "onboard needs its own vehicle mount");

            var onboardCamera = onboard.GetComponent<CinemachineCamera>();
            Assert.That(Mathf.Abs(onboardCamera.Follow.localPosition.x), Is.GreaterThan(0.5f),
                "onboard needs a side-boom mount outside the vehicle geometry");
            Assert.That(onboardCamera.LookAt.localPosition.y,
                Is.LessThan(onboardCamera.Follow.localPosition.y),
                "onboard should look aft along the vehicle, not only into empty sky");

            var groundZoom = anchors.Camera(CameraDirector.GroundTracking)
                .GetComponent<GroundOpticalZoom>();
            Assert.That(groundZoom, Is.Not.Null, "ground tracking needs a long-range optical zoom");
        }

        [Test]
        public void GroundCameraUsesOpticalZoomBelowOneDegreeForDistantTracking()
        {
            var anchors = OpenAndFindAnchors();
            var ground = anchors.Camera(CameraDirector.GroundTracking);

            var opticalZoom = ground.GetComponent("GroundOpticalZoom");
            Assert.That(opticalZoom, Is.Not.Null,
                "ground tracking needs an explicit optical-zoom extension for distant vehicle framing");

            var field = opticalZoom.GetType().GetField("FovRange");
            Assert.That(field, Is.Not.Null, "optical zoom must expose its supported FOV range");
            var range = (Vector2)field.GetValue(opticalZoom);
            Assert.That(range.x, Is.LessThan(1f),
                "range optics need a sub-degree FOV to keep the vehicle readable at altitude");
        }

        [Test]
        public void SceneHostsEveryAlwaysOnEngineeringLayer()
        {
            var anchors = OpenAndFindAnchors();
            foreach (var layer in EngineeringLayerCatalog.Always())
                Assert.That(anchors.LayerHost(layer.Id), Is.Not.Null, layer.Id);
        }

        [Test]
        public void VehicleStagesHaveNoActivePhysicsColliders()
        {
            // Scientific motion is trace-driven; no collider/Rigidbody may drive it.
            var anchors = OpenAndFindAnchors();
            foreach (var stage in new[] { anchors.boosterStage, anchors.sustainerStage })
            {
                Assert.That(stage.GetComponentsInChildren<Collider>(true), Is.Empty, $"{stage.name} has a collider");
                Assert.That(stage.GetComponentsInChildren<Rigidbody>(true), Is.Empty, $"{stage.name} has a Rigidbody");
            }
        }

        [Test]
        public void SceneHasTraceDrivenPlume()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.plume, Is.Not.Null, "plume anchor");
            Assert.That(anchors.plume.GetComponent<ParticleSystem>(), Is.Not.Null, "plume ParticleSystem");
            Assert.That(anchors.plume.GetComponent<PlumeController>(), Is.Not.Null, "plume controller");
            // The plume must start idle — it emits only when the trace says powered.
            Assert.That(anchors.plume.GetComponent<ParticleSystem>().emission.rateOverTime.constant,
                Is.EqualTo(0f), "plume must start with zero emission");
        }

        [Test]
        public void SceneHasWhiteSandsTerrainWithAFlatPadAtOrigin()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.terrain, Is.Not.Null, "terrain anchor");
            var terrain = anchors.terrain.GetComponent<Terrain>();
            Assert.That(terrain, Is.Not.Null, "terrain host lacks a Terrain component");
            Assert.That(terrain.terrainData, Is.Not.Null, "terrain has no TerrainData");
            Assert.That(terrain.terrainData.size.x, Is.GreaterThan(0f), "terrain has zero extent");
            // The launch apron must be flat: sampled height at the origin is ~0.
            float h = terrain.SampleHeight(Vector3.zero) + anchors.terrain.position.y;
            Assert.That(h, Is.LessThan(0.5f), "pad apron at origin must be flat");
        }

        [Test]
        public void SceneHasReviewShellWithLiveUiDocument()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.reviewShell, Is.Not.Null, "review shell anchor");
            var doc = anchors.reviewShell.GetComponent<UnityEngine.UIElements.UIDocument>();
            Assert.That(doc, Is.Not.Null, "review shell lacks a UIDocument");
            Assert.That(doc.visualTreeAsset, Is.Not.Null, "UIDocument has no source UXML");
            Assert.That(doc.panelSettings, Is.Not.Null, "UIDocument has no PanelSettings");
            Assert.That(anchors.reviewShell.GetComponent<ReviewHud>(), Is.Not.Null, "review shell lacks ReviewHud");
        }

        [Test]
        public void UiToolkitWorkbenchAssetsExist()
        {
            Assert.That(System.IO.File.Exists("Assets/Ascent/UI/ReviewWorkbench.uxml"), Is.True);
            Assert.That(System.IO.File.Exists("Assets/Ascent/UI/ReviewWorkbench.uss"), Is.True);
        }
    }
}
