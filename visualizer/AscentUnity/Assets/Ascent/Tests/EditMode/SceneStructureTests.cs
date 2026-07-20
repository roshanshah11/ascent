using System.Linq;
using Ascent.Runtime.Presentation;
using NUnit.Framework;
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
        public void SceneHasFiveCamerasOnePerDirectorId()
        {
            var anchors = OpenAndFindAnchors();
            Assert.That(anchors.cameras.Count, Is.EqualTo(5));
            foreach (var id in CameraDirector.Ids)
            {
                var cam = anchors.Camera(id);
                Assert.That(cam, Is.Not.Null, id);
                Assert.That(cam.GetComponent<Camera>(), Is.Not.Null, $"{id} lacks a Camera");
            }
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
        public void UiToolkitWorkbenchAssetsExist()
        {
            Assert.That(System.IO.File.Exists("Assets/Ascent/UI/ReviewWorkbench.uxml"), Is.True);
            Assert.That(System.IO.File.Exists("Assets/Ascent/UI/ReviewWorkbench.uss"), Is.True);
        }
    }
}
