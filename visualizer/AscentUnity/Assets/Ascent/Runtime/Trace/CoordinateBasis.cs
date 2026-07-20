using UnityEngine;

namespace Ascent.Runtime.Trace
{
    /// <summary>Double-precision vector used for canonical (pre-conversion) trace data.</summary>
    public readonly struct Vector3d
    {
        public readonly double X;
        public readonly double Y;
        public readonly double Z;

        public Vector3d(double x, double y, double z)
        {
            X = x;
            Y = y;
            Z = z;
        }
    }

    /// <summary>Row-major 3×3 double matrix (a canonical attitude basis).</summary>
    public readonly struct Matrix3x3d
    {
        // m[row, col]
        public readonly double M00, M01, M02;
        public readonly double M10, M11, M12;
        public readonly double M20, M21, M22;

        public Matrix3x3d(
            double m00, double m01, double m02,
            double m10, double m11, double m12,
            double m20, double m21, double m22)
        {
            M00 = m00; M01 = m01; M02 = m02;
            M10 = m10; M11 = m11; M12 = m12;
            M20 = m20; M21 = m21; M22 = m22;
        }

        public static readonly Matrix3x3d Identity =
            new Matrix3x3d(1, 0, 0, 0, 1, 0, 0, 0, 1);
    }

    /// <summary>
    /// Converts canonical East-North-Up (ENU) trace data into Unity's left-handed
    /// Y-up space. ENU (E, N, U) maps to Unity (E, U, N): East→X, Up→Y, North→Z.
    /// The attitude conversion applies the same change of basis to the whole
    /// rotation — never a field swap on the quaternion components.
    /// </summary>
    public static class CoordinateBasis
    {
        public static Vector3 Position(Vector3d enu)
        {
            return new Vector3((float)enu.X, (float)enu.Z, (float)enu.Y);
        }

        public static Vector3 Velocity(Vector3d enu) => Position(enu);

        /// <summary>
        /// Convert an ENU rotation matrix to a Unity quaternion via
        /// R_unity = P · R_enu · Pᵀ, where P swaps the Y and Z axes.
        /// </summary>
        public static Quaternion Attitude(Matrix3x3d r)
        {
            // P swaps rows/cols 1 and 2 (Y<->Z). P·R·Pᵀ permutes both indices.
            // Result rows/cols: index 0 stays, 1<-2, 2<-1.
            double u00 = r.M00, u01 = r.M02, u02 = r.M01;
            double u10 = r.M20, u11 = r.M22, u12 = r.M21;
            double u20 = r.M10, u21 = r.M12, u22 = r.M11;
            return MatrixToQuaternion(
                u00, u01, u02,
                u10, u11, u12,
                u20, u21, u22);
        }

        private static Quaternion MatrixToQuaternion(
            double m00, double m01, double m02,
            double m10, double m11, double m12,
            double m20, double m21, double m22)
        {
            double trace = m00 + m11 + m22;
            double qw, qx, qy, qz;
            if (trace > 0.0)
            {
                double s = System.Math.Sqrt(trace + 1.0) * 2.0;
                qw = 0.25 * s;
                qx = (m21 - m12) / s;
                qy = (m02 - m20) / s;
                qz = (m10 - m01) / s;
            }
            else if (m00 > m11 && m00 > m22)
            {
                double s = System.Math.Sqrt(1.0 + m00 - m11 - m22) * 2.0;
                qw = (m21 - m12) / s;
                qx = 0.25 * s;
                qy = (m01 + m10) / s;
                qz = (m02 + m20) / s;
            }
            else if (m11 > m22)
            {
                double s = System.Math.Sqrt(1.0 + m11 - m00 - m22) * 2.0;
                qw = (m02 - m20) / s;
                qx = (m01 + m10) / s;
                qy = 0.25 * s;
                qz = (m12 + m21) / s;
            }
            else
            {
                double s = System.Math.Sqrt(1.0 + m22 - m00 - m11) * 2.0;
                qw = (m10 - m01) / s;
                qx = (m02 + m20) / s;
                qy = (m12 + m21) / s;
                qz = 0.25 * s;
            }
            var q = new Quaternion((float)qx, (float)qy, (float)qz, (float)qw);
            return Quaternion.Normalize(q);
        }
    }
}
