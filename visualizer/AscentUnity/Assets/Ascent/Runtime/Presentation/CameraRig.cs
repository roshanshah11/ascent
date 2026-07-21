using System.Collections.Generic;
using Unity.Cinemachine;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Binds the logical <see cref="CameraDirector"/> to the physical Cinemachine
    /// virtual cameras: selecting a camera id raises that vcam's priority so the
    /// single <see cref="CinemachineBrain"/> blends to it. The director stays pure
    /// logic; this component is the only place that touches Cinemachine.
    /// </summary>
    public sealed class CameraRig : MonoBehaviour
    {
        public const int ActivePriority = 20;
        public const int IdlePriority = 0;

        [Tooltip("Virtual cameras keyed by CameraDirector id.")]
        public List<SceneAnchors.NamedTransform> vcams = new List<SceneAnchors.NamedTransform>();

        private CameraDirector _director;

        /// <summary>Subscribes to a director and snaps to its current selection.</summary>
        public void Bind(CameraDirector director)
        {
            if (_director != null)
                _director.ActiveChanged -= Select;
            _director = director;
            if (_director != null)
            {
                _director.ActiveChanged += Select;
                Select(_director.ActiveId);
            }
        }

        private void OnDestroy()
        {
            if (_director != null)
                _director.ActiveChanged -= Select;
        }

        /// <summary>Raises the selected vcam's priority; lowers the rest.</summary>
        public void Select(string id)
        {
            foreach (var nt in vcams)
            {
                var vcam = nt.transform != null ? nt.transform.GetComponent<CinemachineCamera>() : null;
                if (vcam != null)
                    vcam.Priority = nt.id == id ? ActivePriority : IdlePriority;
            }
        }

        /// <summary>
        /// Pure priority assignment, extracted for testing: the vcam whose id
        /// equals <paramref name="activeId"/> gets <paramref name="active"/>, all
        /// others get <paramref name="idle"/>. Exactly one wins when the id exists.
        /// </summary>
        public static void ApplyPriorities(
            IEnumerable<(string id, CinemachineCamera vcam)> vcams, string activeId, int active, int idle)
        {
            foreach (var (id, vcam) in vcams)
            {
                if (vcam != null)
                    vcam.Priority = id == activeId ? active : idle;
            }
        }
    }
}
