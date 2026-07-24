using System;
using System.Globalization;
using Ascent.Runtime.Trace;
using UnityEngine;
using UnityEngine.UIElements;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// Mounts the authored review workbench UXML into a live <see cref="UIDocument"/>
    /// and binds it to a <see cref="ReviewWorkbench"/> controller: it builds the
    /// engineering-layer rail, the camera rail, and the time-positioned event
    /// track from the accepted trace, wires the clean-view toggle, and pushes
    /// controller snapshots into the State/Evidence panels and the transport
    /// readouts. Nothing here is a source of flight state — every value it shows
    /// comes from the controller, which reads only the trace.
    ///
    /// The tree-builder methods (<see cref="PopulateLayerRail"/>,
    /// <see cref="PopulateCameraList"/>, <see cref="PopulateEventTrack"/>,
    /// <see cref="RenderState"/>) operate on a plain <see cref="VisualElement"/> so
    /// they can be exercised headlessly against a cloned UXML tree without a live
    /// panel.
    /// </summary>
    [RequireComponent(typeof(UIDocument))]
    public sealed class ReviewHud : MonoBehaviour
    {
        private UIDocument _document;
        private ReviewWorkbench _workbench;
        private VisualElement _root;

        /// <summary>Binds the HUD to a controller once a trace has been accepted.</summary>
        public void Bind(VisualElement root, ReviewWorkbench workbench)
        {
            _root = root ?? throw new ArgumentNullException(nameof(root));
            _workbench = workbench ?? throw new ArgumentNullException(nameof(workbench));

            var layerList = root.Q<VisualElement>("layer-list");
            if (layerList != null)
                PopulateLayerRail(layerList, workbench, () => RenderState(root, workbench));

            var cameraList = root.Q<VisualElement>("camera-list");
            if (cameraList != null)
                PopulateCameraList(cameraList, workbench, () => SyncActiveCamera(root, workbench));
            // The camera can also change from the keyboard (digits 1–5) in the
            // bootstrap; keep the rail's active chip and label in sync either way.
            workbench.Cameras.ActiveChanged += OnActiveCameraChanged;

            var eventTrack = root.Q<VisualElement>("event-track");
            if (eventTrack != null)
                PopulateEventTrack(eventTrack, workbench.Trace, (kind, _) =>
                {
                    workbench.SeekEvent(kind);
                    RenderState(root, workbench);
                });

            var cleanView = root.Q<Toggle>("clean-view");
            if (cleanView != null)
            {
                cleanView.value = workbench.CleanView;
                cleanView.RegisterValueChangedCallback(evt =>
                {
                    workbench.SetCleanView(evt.newValue);
                    RenderState(root, workbench);
                });
            }

            var playPause = root.Q<Button>("play-pause");
            if (playPause != null)
                playPause.clicked += () =>
                {
                    if (workbench.Clock.IsPlaying) workbench.Clock.Pause();
                    else workbench.Clock.Play();
                    RenderState(root, workbench);
                };

            // The top-bar hash is fixed for the accepted trace: bind it at bind
            // time so it can never remain the idle "trace —" placeholder once Ready.
            var traceHash = root.Q<Label>("trace-hash");
            if (traceHash != null)
                traceHash.text = $"trace {Short(workbench.Trace.TraceHash, 16)}…";

            // Evidence is fixed for the accepted trace, so build it once here.
            var evidenceDrawer = root.Q<VisualElement>("evidence-drawer");
            if (evidenceDrawer != null)
                RenderEvidence(evidenceDrawer, workbench.Trace);

            SyncActiveCamera(root, workbench);
            RenderState(root, workbench);
        }

        private void OnActiveCameraChanged(string _)
        {
            if (_root != null && _workbench != null)
                Refresh();
        }

        private void OnEnable()
        {
            _document = GetComponent<UIDocument>();
            TryBindDocument();
        }

        private void OnDisable()
        {
            if (_workbench != null)
                _workbench.Cameras.ActiveChanged -= OnActiveCameraChanged;
        }

        private void TryBindDocument()
        {
            if (_document == null || _workbench == null)
                return;
            var root = _document.rootVisualElement;
            if (root != null)
                Bind(root, _workbench);
        }

        /// <summary>Assigns the controller; binds immediately if the panel is live.</summary>
        public void SetWorkbench(ReviewWorkbench workbench)
        {
            _workbench = workbench;
            TryBindDocument();
        }

        private void Update()
        {
            if (_root != null && _workbench != null && _workbench.Clock.IsPlaying)
                RenderState(_root, _workbench);
        }

        /// <summary>
        /// Re-syncs the whole panel to the controller after a change driven from
        /// outside the HUD (keyboard transport, camera digits, clean-view key): the
        /// clean-view toggle, the active camera, and the readouts. Used by the
        /// bootstrap so keyboard actions stay reflected in the panel while paused.
        /// </summary>
        public void Refresh()
        {
            if (_root == null || _workbench == null)
                return;
            var cleanView = _root.Q<Toggle>("clean-view");
            if (cleanView != null)
                cleanView.SetValueWithoutNotify(_workbench.CleanView);
            SyncActiveCamera(_root, _workbench);
            RenderState(_root, _workbench);
        }

        /// <summary>
        /// Builds one <see cref="Toggle"/> per always-on engineering layer, each
        /// reflecting current visibility and toggling the controller's layer set.
        /// Returns the number of toggles created.
        /// </summary>
        public static int PopulateLayerRail(VisualElement layerList, ReviewWorkbench workbench, Action onChanged)
        {
            layerList.Clear();
            int count = 0;
            foreach (var layer in EngineeringLayerCatalog.Always())
            {
                var id = layer.Id;
                var toggle = new Toggle(PrettyLayer(id)) { name = $"layer-{id}", value = true };
                toggle.RegisterValueChangedCallback(_ =>
                {
                    workbench.ToggleLayer(id);
                    onChanged?.Invoke();
                });
                layerList.Add(toggle);
                count++;
            }
            return count;
        }

        /// <summary>
        /// Builds one selectable chip per camera in <see cref="CameraDirector.Ids"/>,
        /// labeled with its digit (1–5) and name. Clicking a chip switches the
        /// controller's active camera. Returns the number of chips created.
        /// </summary>
        public static int PopulateCameraList(VisualElement cameraList, ReviewWorkbench workbench, Action onChanged)
        {
            cameraList.Clear();
            int count = 0;
            foreach (var id in CameraDirector.Ids)
            {
                var captured = id;
                int digit = count + 1;
                var chip = new Button { name = $"camera-{id}", text = $"{digit}  {PrettyCamera(id)}" };
                chip.AddToClassList("camera-button");
                chip.clicked += () => ActivateCameraChip(workbench, captured, onChanged);
                cameraList.Add(chip);
                count++;
            }
            return count;
        }

        /// <summary>The action a camera chip runs when clicked: switch the active
        /// camera then let the caller re-sync dependent UI. Exposed as a seam so the
        /// click behavior is verifiable without a live panel to dispatch events.</summary>
        public static void ActivateCameraChip(ReviewWorkbench workbench, string id, Action onChanged)
        {
            workbench.Cameras.SwitchTo(id);
            onChanged?.Invoke();
        }

        /// <summary>
        /// Builds one seek marker per trace event, pinned along the scrubber by the
        /// event's fraction of the trace timeline and labeled with its kind and
        /// exact Rust time. Clicking invokes <paramref name="onSeek"/> with the
        /// event kind and time. Returns the number of markers created.
        /// </summary>
        public static int PopulateEventTrack(VisualElement track, FlightTrace trace, Action<string, double> onSeek)
        {
            track.Clear();
            if (trace == null)
                return 0;
            int count = 0;
            foreach (var e in trace.Events)
            {
                var kind = e.Kind;
                var t = e.TimeSeconds;
                var marker = new Button { name = $"event-{count}", text = $"{kind}  t={t.ToString("0.###", CultureInfo.InvariantCulture)}s" };
                marker.AddToClassList("event-marker");
                marker.style.left = Length.Percent(FractionOfTimeline(trace, t) * 100f);
                marker.clicked += () => onSeek?.Invoke(kind, t);
                track.Add(marker);
                count++;
            }
            return count;
        }

        /// <summary>
        /// Pushes the controller snapshot into the transport readouts, the State
        /// panel (trace-graded rows), and the scrubber progress fill. Evidence is
        /// built once at bind time (it is fixed for the accepted trace).
        /// </summary>
        public static void RenderState(VisualElement root, ReviewWorkbench workbench)
        {
            var s = workbench.Snapshot();

            var status = root.Q<Label>("status");
            if (status != null)
                status.text = s.IsPlaying ? $"Playing · cam {s.CameraId}" : $"Paused · cam {s.CameraId}";

            var clock = root.Q<Label>("clock");
            if (clock != null)
                clock.text = $"t = {s.TimeSeconds.ToString("0.000", CultureInfo.InvariantCulture)} s";

            var rate = root.Q<Label>("rate");
            if (rate != null)
                rate.text = $"{s.Rate.ToString("0.###", CultureInfo.InvariantCulture)}x";

            var stateHost = root.Q<VisualElement>("state-readouts");
            if (stateHost != null)
                RenderStateReadouts(stateHost, workbench.Trace, s.TimeSeconds);

            var fill = root.Q<VisualElement>("scrubber-fill");
            if (fill != null)
                fill.style.width = Length.Percent(FractionOfTimeline(workbench.Trace, s.TimeSeconds) * 100f);
        }

        /// <summary>
        /// Rebuilds the State panel from <see cref="FlightReadout.StateAt"/>: one
        /// row per readout with its label, value, unit, and an evidence-graded
        /// provenance chip. Returns the number of rows written.
        /// </summary>
        public static int RenderStateReadouts(VisualElement host, FlightTrace trace, double t)
        {
            host.Clear();
            if (trace == null)
                return 0;
            int count = 0;
            foreach (var r in FlightReadout.StateAt(trace, t))
            {
                var row = new VisualElement { name = $"state-{r.Label.Replace(' ', '-').ToLowerInvariant()}" };
                row.AddToClassList("state-row");

                var label = new Label(r.Label);
                label.AddToClassList("state-label");
                var value = new Label(r.Value);
                value.AddToClassList("state-value");
                var unit = new Label(r.Unit);
                unit.AddToClassList("state-unit");
                var chip = new VisualElement();
                chip.AddToClassList("prov-chip");
                chip.AddToClassList(ChipClass(r.Provenance));

                row.Add(label);
                row.Add(value);
                row.Add(unit);
                row.Add(chip);
                host.Add(row);
                count++;
            }
            return count;
        }

        /// <summary>
        /// Builds the Evidence panel from the accepted trace: the SHA-256 hash it
        /// was validated against, the acceptance status, the sample and event
        /// counts, and a note explaining the provenance grading. Returns the number
        /// of rows written (excluding the note).
        /// </summary>
        public static int RenderEvidence(VisualElement host, FlightTrace trace)
        {
            host.Clear();
            if (trace == null)
                return 0;

            int rows = 0;
            rows += EvidenceRow(host, "Trace SHA-256", $"{Short(trace.TraceHash, 16)}…", ok: false);
            rows += EvidenceRow(host, "Validation", "accepted · hash-checked", ok: true);
            rows += EvidenceRow(host, "Samples", trace.SampleCount.ToString(CultureInfo.InvariantCulture), ok: false);
            rows += EvidenceRow(host, "Events", trace.Events.Count.ToString(CultureInfo.InvariantCulture), ok: false);

            var note = new Label("Provenance chips: cyan = direct from trace, muted = derived, amber = approximation.");
            note.AddToClassList("evidence-note");
            host.Add(note);
            return rows;
        }

        /// <summary>
        /// Updates the active-camera label and the pressed state of the camera
        /// chips to match the controller's active camera.
        /// </summary>
        public static void SyncActiveCamera(VisualElement root, ReviewWorkbench workbench)
        {
            var activeId = workbench.Cameras.ActiveId;

            var label = root.Q<Label>("active-camera");
            if (label != null)
                label.text = $"active: {PrettyCamera(activeId)}";

            foreach (var chip in root.Query<Button>(className: "camera-button").ToList())
                chip.EnableInClassList("active", chip.name == $"camera-{activeId}");
        }

        private static int EvidenceRow(VisualElement host, string key, string value, bool ok)
        {
            var row = new VisualElement { name = $"evidence-{key.Replace(' ', '-').ToLowerInvariant()}" };
            row.AddToClassList("evidence-row");
            var k = new Label(key);
            k.AddToClassList("evidence-key");
            var v = new Label(value);
            v.AddToClassList(ok ? "evidence-ok" : "evidence-val");
            row.Add(k);
            row.Add(v);
            host.Add(row);
            return 1;
        }

        private static string ChipClass(EvidencePolicy p)
        {
            switch (p)
            {
                case EvidencePolicy.DirectFromTrace: return "prov-direct";
                case EvidencePolicy.DerivedFromTrace: return "prov-derived";
                case EvidencePolicy.Approximate: return "prov-approx";
                default: return "prov-derived";
            }
        }

        /// <summary>The event's position as a 0..1 fraction of the trace timeline.</summary>
        private static float FractionOfTimeline(FlightTrace trace, double t)
        {
            if (trace == null)
                return 0f;
            double span = trace.EndSeconds - trace.StartSeconds;
            if (span <= 0.0)
                return 0f;
            return Mathf.Clamp01((float)((t - trace.StartSeconds) / span));
        }

        private static string PrettyCamera(string id)
        {
            switch (id)
            {
                case CameraDirector.Pad: return "Pad";
                case CameraDirector.Chase: return "Chase";
                case CameraDirector.Onboard: return "Onboard";
                case CameraDirector.GroundTracking: return "Ground";
                case CameraDirector.Inspection: return "Inspection";
                default: return id;
            }
        }

        private static string PrettyLayer(string id)
        {
            switch (id)
            {
                case EngineeringLayerCatalog.Trajectory: return "Trajectory";
                case EngineeringLayerCatalog.Velocity: return "Velocity vector";
                case EngineeringLayerCatalog.BodyAxes: return "Body axes";
                case EngineeringLayerCatalog.Attitude: return "Attitude heading";
                case EngineeringLayerCatalog.StageState: return "Stage state";
                case EngineeringLayerCatalog.Events: return "Event markers";
                default: return id;
            }
        }

        private static string Short(string s, int n) =>
            string.IsNullOrEmpty(s) ? "" : s.Substring(0, Math.Min(n, s.Length));
    }
}
