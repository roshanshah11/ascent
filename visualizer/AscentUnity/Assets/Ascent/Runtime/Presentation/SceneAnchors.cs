using System.Collections.Generic;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Marks the well-known structural roots of the Black Brant IX review scene
    /// so controllers and tests can find them without brittle name lookups. The
    /// scene builder wires these; presentation reads them. Holds no canonical
    /// state — trajectory/attitude come only from the accepted <c>FlightTrace</c>.
    /// </summary>
    public sealed class SceneAnchors : MonoBehaviour
    {
        [Tooltip("Root of the two separable stages.")]
        public Transform vehicleRoot;

        [Tooltip("Booster (Terrier) stage root.")]
        public Transform boosterStage;

        [Tooltip("Sustainer (Black Brant) stage root.")]
        public Transform sustainerStage;

        [Tooltip("Parent of the five camera rigs, indexed by CameraDirector.Ids.")]
        public Transform cameraRig;

        [Tooltip("Parent of the per-layer host objects.")]
        public Transform engineeringLayers;

        [Tooltip("Named camera anchors keyed by camera id.")]
        public List<NamedTransform> cameras = new List<NamedTransform>();

        [Tooltip("Named engineering-layer host objects keyed by layer id.")]
        public List<NamedTransform> layerHosts = new List<NamedTransform>();

        public Transform Camera(string id) => Lookup(cameras, id);
        public Transform LayerHost(string id) => Lookup(layerHosts, id);

        private static Transform Lookup(List<NamedTransform> list, string id)
        {
            foreach (var nt in list)
                if (nt.id == id)
                    return nt.transform;
            return null;
        }

        [System.Serializable]
        public struct NamedTransform
        {
            public string id;
            public Transform transform;
        }
    }
}
