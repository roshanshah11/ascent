using System.Collections.Generic;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Trace;
using UnityEngine;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Renders the engineering overlays into the scene and shows/hides them from
    /// the review controller's layer-visibility set. The overlays are built from
    /// the accepted <see cref="FlightTrace"/> — the trajectory line and event
    /// markers from the trace's own samples/events, the velocity vector and body
    /// axes from the trace sampled at the current review time. Nothing here is a
    /// source of canonical state; it is a faithful drawing of the trace.
    ///
    /// Layer id → a host <see cref="GameObject"/> that is toggled active. Clean
    /// view is just the empty visible set, so it hides every overlay without any
    /// special case here.
    /// </summary>
    public sealed class EngineeringLayersView : MonoBehaviour
    {
        [Tooltip("Well-known scene roots (vehicle + per-layer hosts).")]
        public SceneAnchors anchors;

        [Tooltip("A scene-authored HDRP/Lit material retained by the player build. Runtime overlay materials clone this template so they cannot resolve to stripped magenta shaders.")]
        public Material overlayMaterialTemplate;

        private FlightTrace _trace;
        private readonly Dictionary<string, GameObject> _hosts = new Dictionary<string, GameObject>();

        // Dynamic overlay pieces refreshed each frame from the trace.
        private LineRenderer _velocity;
        private readonly LineRenderer[] _axes = new LineRenderer[3];
        private LineRenderer _attitude;
        private Transform _stageMarker;
        private Material _boosterMat;
        private Material _sustainerMat;

        private const float VelocityScale = 0.06f;   // metres of arrow per m/s
        private const float AxisLength = 3.0f;

        /// <summary>Builds all overlay geometry from the trace. Call once, after Ready.</summary>
        public void Initialize(FlightTrace trace)
        {
            _trace = trace;
            if (anchors == null)
                return;

            BuildTrajectory();
            BuildEventMarkers();
            BuildVelocity();
            BuildBodyAxes();
            BuildAttitude();
            BuildStageState();
        }

        /// <summary>Show exactly the layers named in <paramref name="visibleLayers"/>.</summary>
        public void ApplyVisibility(IReadOnlyList<string> visibleLayers)
        {
            var visible = new HashSet<string>(visibleLayers ?? System.Array.Empty<string>());
            foreach (var kv in _hosts)
                if (kv.Value != null)
                    kv.Value.SetActive(visible.Contains(kv.Key));
        }

        /// <summary>Refresh the trace-time-dependent overlays (velocity, axes, stage marker).</summary>
        public void UpdateAt(double t)
        {
            if (_trace == null || anchors == null || anchors.vehicleRoot == null)
                return;
            var vehicle = anchors.vehicleRoot;
            Vector3 origin = vehicle.position;

            if (_velocity != null)
            {
                var v = _trace.VelocityAt(t);
                Vector3 tip = v.IsGap ? origin : origin + v.Value * VelocityScale;
                _velocity.SetPosition(0, origin);
                _velocity.SetPosition(1, tip);
            }

            if (_axes[0] != null)
            {
                SetAxis(_axes[0], origin, vehicle.right);
                SetAxis(_axes[1], origin, vehicle.up);
                SetAxis(_axes[2], origin, vehicle.forward);
            }

            if (_attitude != null)
            {
                _attitude.SetPosition(0, origin);
                _attitude.SetPosition(1, origin + vehicle.up * (AxisLength * 2.5f));
            }

            if (_stageMarker != null)
            {
                _stageMarker.position = origin;
                int stage = FlightReadout.StageNumber(_trace.Events, t);
                var r = _stageMarker.GetComponent<Renderer>();
                if (r != null)
                    r.sharedMaterial = stage <= 1 ? _boosterMat : _sustainerMat;
            }
        }

        private void SetAxis(LineRenderer lr, Vector3 origin, Vector3 dir)
        {
            lr.SetPosition(0, origin);
            lr.SetPosition(1, origin + dir.normalized * AxisLength);
        }

        // --- Overlay construction -------------------------------------------------

        private GameObject Host(string id)
        {
            var host = anchors.LayerHost(id);
            GameObject go = host != null ? host.gameObject : new GameObject($"Layer_{id}");
            if (host == null)
                go.transform.SetParent(anchors.engineeringLayers, false);
            _hosts[id] = go;
            return go;
        }

        private void BuildTrajectory()
        {
            var host = Host(EngineeringLayerCatalog.Trajectory);
            int n = _trace.SampleCount;
            if (n < 2)
                return;
            // The trajectory is an annotation, not a proxy for the vehicle. Keep it
            // visible in the pad shot without covering the 0.66 m airframe.
            var lr = NewLine(host.transform, "Trajectory", Cyan, 0.08f);
            // Sample the path densely but capped so very long traces stay cheap.
            int step = Mathf.Max(1, n / 512);
            var pts = new List<Vector3>();
            for (int i = 0; i < n; i += step)
            {
                double t = System.Math.Min(_trace.EndSeconds,
                    _trace.StartSeconds + (i / (double)(n - 1)) * (_trace.EndSeconds - _trace.StartSeconds));
                var p = _trace.PositionAt(t);
                if (!p.IsGap)
                    pts.Add(p.Value);
            }
            lr.positionCount = pts.Count;
            lr.SetPositions(pts.ToArray());
        }

        private void BuildEventMarkers()
        {
            var host = Host(EngineeringLayerCatalog.Events);
            foreach (var e in _trace.Events)
            {
                if (e == null)
                    continue;
                var p = _trace.PositionAt(e.TimeSeconds);
                if (p.IsGap)
                    continue;
                var marker = GameObject.CreatePrimitive(PrimitiveType.Sphere);
                marker.name = $"Event_{e.Kind}";
                DestroyCollider(marker);
                marker.transform.SetParent(host.transform, false);
                marker.transform.position = p.Value;
                // Small annotation dots. Authored large (2.2) they dominated the pad
                // camera and occluded the slender airframe; the engineering-layer
                // information role is preserved at a size that reads as a marker, not
                // a scene-filling sphere.
                marker.transform.localScale = Vector3.one * 0.6f;
                marker.GetComponent<Renderer>().sharedMaterial = UnlitMat(Cyan, "EventMarkerMat");
            }
        }

        private void BuildVelocity()
        {
            var host = Host(EngineeringLayerCatalog.Velocity);
            _velocity = NewLine(host.transform, "VelocityVector", Amber, 0.5f);
            _velocity.positionCount = 2;
        }

        private void BuildBodyAxes()
        {
            var host = Host(EngineeringLayerCatalog.BodyAxes);
            _axes[0] = NewLine(host.transform, "AxisX", new Color(0.95f, 0.35f, 0.35f), 0.35f);
            _axes[1] = NewLine(host.transform, "AxisY", new Color(0.45f, 0.9f, 0.45f), 0.35f);
            _axes[2] = NewLine(host.transform, "AxisZ", new Color(0.4f, 0.6f, 1f), 0.35f);
            foreach (var a in _axes)
                a.positionCount = 2;
        }

        private void BuildAttitude()
        {
            var host = Host(EngineeringLayerCatalog.Attitude);
            _attitude = NewLine(host.transform, "AttitudeHeading", new Color(0.9f, 0.92f, 0.96f), 0.3f);
            _attitude.positionCount = 2;
        }

        private void BuildStageState()
        {
            var host = Host(EngineeringLayerCatalog.StageState);
            _boosterMat = UnlitMat(Cyan, "StageBoosterMat");
            _sustainerMat = UnlitMat(Amber, "StageSustainerMat");
            var ring = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            ring.name = "StageMarker";
            DestroyCollider(ring);
            ring.transform.SetParent(host.transform, false);
            // Compact stage-state dot near the vehicle rather than a 1.4 sphere that
            // swallowed the nose in the pad camera.
            ring.transform.localScale = Vector3.one * 0.6f;
            ring.GetComponent<Renderer>().sharedMaterial = _boosterMat;
            _stageMarker = ring.transform;
        }

        // --- Rendering helpers ----------------------------------------------------

        private static readonly Color Cyan = new Color(0.30f, 0.80f, 0.95f);
        private static readonly Color Amber = new Color(0.98f, 0.72f, 0.30f);

        private LineRenderer NewLine(Transform parent, string name, Color color, float width)
        {
            var go = new GameObject(name);
            go.transform.SetParent(parent, false);
            var lr = go.AddComponent<LineRenderer>();
            lr.useWorldSpace = true;
            lr.widthMultiplier = width;
            lr.numCapVertices = 2;
            lr.material = UnlitMat(color, $"{name}Mat");
            lr.startColor = color;
            lr.endColor = color;
            lr.positionCount = 0;
            return lr;
        }

        /// <summary>
        /// An unlit, self-lit material that reads the same in any pipeline. Prefers
        /// HDRP/Unlit (with emissive so it stays visible against bright terrain),
        /// then falls back through built-in unlit shaders that always exist.
        /// </summary>
        private Material UnlitMat(Color color, string name)
        {
            // PlayerBuilder explicitly retains HDRP/Unlit. Unlike the airframe's
            // HDRP/Lit shader, it does not shade a camera-facing trajectory black.
            var shader = Shader.Find("HDRP/Unlit");
            var mat = shader != null
                ? new Material(shader) { name = name }
                : overlayMaterialTemplate != null
                    ? new Material(overlayMaterialTemplate) { name = name }
                    : new Material(Shader.Find("Unlit/Color")) { name = name };
            if (mat.HasProperty("_UnlitColor"))
                mat.SetColor("_UnlitColor", color);
            if (mat.HasProperty("_Color"))
                mat.SetColor("_Color", color);
            if (mat.HasProperty("_BaseColor"))
                mat.SetColor("_BaseColor", color);
            return mat;
        }

        private static void DestroyCollider(GameObject go)
        {
            var col = go.GetComponent<Collider>();
            if (col != null)
                Destroy(col);
        }
    }
}
