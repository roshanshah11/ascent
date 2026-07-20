using Ascent.Runtime.Trace;
using NUnit.Framework;
using UnityEngine;

namespace Ascent.Tests.EditMode
{
    public class CoordinateBasisTests
    {
        [Test]
        public void EnuBasisMapsAxesWithoutQuaternionFieldSwaps()
        {
            Assert.That(CoordinateBasis.Position(new Vector3d(1, 0, 0)), Is.EqualTo(new Vector3(1, 0, 0)));
            Assert.That(CoordinateBasis.Position(new Vector3d(0, 1, 0)), Is.EqualTo(new Vector3(0, 0, 1)));
            Assert.That(CoordinateBasis.Position(new Vector3d(0, 0, 1)), Is.EqualTo(new Vector3(0, 1, 0)));
            Assert.That(CoordinateBasis.Attitude(Matrix3x3d.Identity), Is.EqualTo(Quaternion.identity));
        }

        [Test]
        public void AttitudeOfEnuYawRotatesEastToNorthInUnitySpace()
        {
            // 90° yaw about Up (ENU +Z) sends the East axis to North. After the
            // basis change, applying the Unity quaternion to Unity-East (+X) must
            // yield Unity-North (+Z).
            double c = 0.0, s = 1.0; // cos90, sin90
            var yaw = new Matrix3x3d(
                c, -s, 0,
                s, c, 0,
                0, 0, 1);
            var q = CoordinateBasis.Attitude(yaw);
            var rotatedEast = q * new Vector3(1, 0, 0);
            Assert.That(Vector3.Distance(rotatedEast, new Vector3(0, 0, 1)), Is.LessThan(1e-4f));
        }
    }
}
