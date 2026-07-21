using System.Collections.Generic;
using Ascent.Runtime.Presentation;
using NUnit.Framework;
using Unity.Cinemachine;
using UnityEngine;

namespace Ascent.Tests.EditMode
{
    /// <summary>
    /// Selecting a camera id must raise exactly that virtual camera's priority so
    /// the Cinemachine brain blends to it, and lower every other.
    /// </summary>
    public class CameraRigTests
    {
        private readonly List<GameObject> _spawned = new List<GameObject>();

        private (string, CinemachineCamera) Vcam(string id)
        {
            var go = new GameObject($"vcam_{id}");
            _spawned.Add(go);
            return (id, go.AddComponent<CinemachineCamera>());
        }

        [TearDown]
        public void TearDown()
        {
            foreach (var go in _spawned)
                Object.DestroyImmediate(go);
            _spawned.Clear();
        }

        [Test]
        public void ActiveVcamGetsHighestPriority()
        {
            var vcams = new List<(string, CinemachineCamera)>
            {
                Vcam(CameraDirector.Pad),
                Vcam(CameraDirector.Chase),
                Vcam(CameraDirector.Inspection),
            };

            CameraRig.ApplyPriorities(vcams, CameraDirector.Chase, CameraRig.ActivePriority, CameraRig.IdlePriority);

            Assert.That(vcams[0].Item2.Priority.Value, Is.EqualTo(CameraRig.IdlePriority), "pad");
            Assert.That(vcams[1].Item2.Priority.Value, Is.EqualTo(CameraRig.ActivePriority), "chase (active)");
            Assert.That(vcams[2].Item2.Priority.Value, Is.EqualTo(CameraRig.IdlePriority), "inspection");
        }

        [Test]
        public void ExactlyOneVcamIsActiveAcrossEveryId()
        {
            var vcams = new List<(string, CinemachineCamera)>();
            foreach (var id in CameraDirector.Ids)
                vcams.Add(Vcam(id));

            foreach (var active in CameraDirector.Ids)
            {
                CameraRig.ApplyPriorities(vcams, active, CameraRig.ActivePriority, CameraRig.IdlePriority);
                int high = 0;
                foreach (var (id, vcam) in vcams)
                    if (vcam.Priority.Value == CameraRig.ActivePriority) high++;
                Assert.That(high, Is.EqualTo(1), $"exactly one active when {active} selected");
            }
        }
    }
}
