using Unity.Cinemachine;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Frames a fixed-height launch vehicle from a stationary range camera.
    /// Unlike a chase camera, the camera stays at the range; this extension only
    /// controls its lens. A sub-degree minimum field of view is required for an
    /// honest optical track once the vehicle is tens of kilometres downrange.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class GroundOpticalZoom : CinemachineExtension
    {
        [Tooltip("Approximate vehicle height used to frame the optical track, in metres.")]
        public float TargetHeightMeters = 12f;

        [Tooltip("Vertical fraction of the frame occupied by the vehicle when the lens is not clamped.")]
        [Range(0.05f, 0.95f)]
        public float TargetScreenHeight = 0.42f;

        [Tooltip("Supported vertical FOV range in degrees. The low bound permits range-optics framing.")]
        public Vector2 FovRange = new Vector2(0.2f, 35f);

        protected override void PostPipelineStageCallback(
            CinemachineVirtualCameraBase vcam,
            CinemachineCore.Stage stage,
            ref CameraState state,
            float deltaTime)
        {
            if (stage != CinemachineCore.Stage.Finalize)
                return;

            var lookAt = vcam.LookAt;
            if (lookAt == null)
                return;

            float distance = Vector3.Distance(state.GetCorrectedPosition(), lookAt.position);
            if (distance <= Mathf.Epsilon)
                return;

            float unclampedFov = VerticalFieldOfView(distance, TargetHeightMeters, TargetScreenHeight);
            var lens = state.Lens;
            lens.FieldOfView = Mathf.Clamp(unclampedFov, FovRange.x, FovRange.y);
            state.Lens = lens;
        }

        /// <summary>Calculates the vertical FOV that gives a target a fixed screen height.</summary>
        public static float VerticalFieldOfView(float distance, float targetHeight, float screenHeight)
        {
            if (distance <= Mathf.Epsilon || targetHeight <= 0f || screenHeight <= 0f)
                return 179f;
            return 2f * Mathf.Atan(targetHeight / (2f * distance * screenHeight)) * Mathf.Rad2Deg;
        }

        private void OnValidate()
        {
            TargetHeightMeters = Mathf.Max(0.01f, TargetHeightMeters);
            TargetScreenHeight = Mathf.Clamp(TargetScreenHeight, 0.05f, 0.95f);
            FovRange.x = Mathf.Clamp(FovRange.x, 0.01f, 179f);
            FovRange.y = Mathf.Clamp(FovRange.y, FovRange.x, 179f);
        }
    }
}
