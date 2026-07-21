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
    /// engineering-layer rail and the semantic event track from the accepted
    /// trace, wires the clean-view toggle, and pushes controller snapshots into
    /// the status/clock/rate readouts. Nothing here is a source of flight state —
    /// every value it shows comes from the controller, which reads only the trace.
    ///
    /// The tree-builder methods (<see cref="PopulateLayerRail"/>,
    /// <see cref="PopulateEventTrack"/>, <see cref="RenderState"/>) operate on a
    /// plain <see cref="VisualElement"/> so they can be exercised headlessly
    /// against a cloned UXML tree without a live panel.
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

            RenderState(root, workbench);
        }

        private void OnEnable()
        {
            _document = GetComponent<UIDocument>();
            TryBindDocument();
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
                var toggle = new Toggle(id) { name = $"layer-{id}", value = true };
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
        /// Builds one seek <see cref="Button"/> per trace event in time order,
        /// labeled with the event kind and its exact Rust time. Clicking invokes
        /// <paramref name="onSeek"/> with the event kind and time. Returns the
        /// number of markers created.
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
                marker.clicked += () => onSeek?.Invoke(kind, t);
                track.Add(marker);
                count++;
            }
            return count;
        }

        /// <summary>Pushes the controller snapshot into the readout labels.</summary>
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
        }
    }
}
