using System;
using System.Collections;
using System.Globalization;
using System.IO;
using Ascent.Runtime.Bridge;
using Ascent.Runtime.Trace;
using UnityEngine;
using UnityEngine.UIElements;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// The single runtime driver of the Black Brant IX review. On scene start it
    /// launches the read-only Rust bridge, runs the reference mission to an
    /// accepted (hash-validated) <see cref="FlightTrace"/>, builds the
    /// <see cref="ReviewWorkbench"/> controller, and binds it to the live HUD, the
    /// camera rig, and the engineering overlays — the trace is the only source of
    /// flight state. Once the trace is accepted the bridge is shut down (playback
    /// is pure seeking over an immutable trace), so no child process outlives trace
    /// acceptance.
    ///
    /// Keyboard: Space toggles play/pause; digits 1–5 select cameras; C toggles the
    /// clean view (hiding every engineering overlay and restoring it). While playing
    /// it advances the review clock and drives the vehicle pose, exhaust plume, and
    /// overlay geometry purely from the trace.
    ///
    /// Launched with <c>-ascent-review-smoke</c> it self-verifies the path headlessly
    /// (bridge launch, accepted hash, HUD ready, Space advancing playback, clean
    /// shutdown) then writes JSON and quits. Launched with
    /// <c>-ascent-review-visual</c> it advances to a nonzero powered-flight time,
    /// asserts the State/Evidence panels are populated and the scene actually
    /// renders (framebuffer luminance), captures a 1920×1080 screenshot, writes an
    /// acceptance JSON, and quits with a pass/fail code.
    /// </summary>
    public sealed class ReviewBootstrap : MonoBehaviour
    {
        public const string MissionId = "nasa.black-brant-ix.reference";
        private const double FrameRate = 60.0;
        private const float ConnectTimeoutSeconds = 30f;
        private const int SmokeAdvanceFrames = 20;
        private const int VisualWidth = 1920;
        private const int VisualHeight = 1080;

        // Visual-gate tuning.
        private const int VisualSettleFrames = 16;      // let HDRP exposure/AA settle after the seek
        private const double VisualLuminanceFloor = 0.06; // below this the viewport is effectively black

        public SceneAnchors anchors;
        public ReviewHud hud;
        public PlumeController plume;
        public EngineeringLayersView layers;

        private BridgeProcess _bridge;
        private MissionRunner _runner;
        private ReviewWorkbench _workbench;
        private float _deadline;
        private bool _bound;
        private bool _failed;
        private int _lastLayerRevision = -1;

        // Smoke-mode state.
        private bool _smoke;
        private bool _smokeDone;
        private string _smokeOut;
        private double _smokeStartTime;
        private int _smokeFrames;

        // Visual-gate state.
        private bool _visual;
        private bool _visualDone;
        private bool _visualCapturing;
        private string _visualOut;
        private string _visualShot;
        private double _visualTargetTime;
        private string _visualCameraId = CameraDirector.Pad;
        private double? _visualRequestedTime;
        private int _visualFrames;

        private void Start()
        {
            ParseArgs();

            // The packaged visual/smoke gates may run beside an interactive review
            // player. Keep pumping the bridge even when this window is not focused.
            Application.runInBackground = true;

            var path = BridgeLocator.Resolve();
            if (path == null)
            {
                Fail("bridge executable not found (StreamingAssets or target/debug)");
                return;
            }
            BridgeLocator.EnsureExecutable(path);

            try
            {
                // maxRestarts: 0 — the review runs one deterministic mission then
                // shuts the bridge down for good (playback is pure seeking). Auto
                // restart would misread our own intentional shutdown as a crash and
                // respawn a child, leaving an orphan; we never want that here.
                _bridge = new BridgeProcess(path, maxRestarts: 0);
                _bridge.Start();
                _runner = new MissionRunner(_bridge, MissionId);
                _runner.Begin();
                _deadline = Time.realtimeSinceStartup + ConnectTimeoutSeconds;
                SetLabel("status", "Connecting…");
            }
            catch (Exception ex)
            {
                Fail($"bridge launch failed: {ex.Message}");
            }
        }

        private void Update()
        {
            if (_failed || _smokeDone || _visualDone)
                return;

            if (!_bound)
            {
                PumpUntilReady();
                return;
            }

            // The visual gate drives its own deterministic seek + capture; it must
            // not be perturbed by clock advance or live keyboard input.
            if (_visual)
            {
                DriveVisual();
                return;
            }

            // Ready: keyboard transport (smoke mode scripts its own actions).
            if (!_smoke)
                HandleInput();

            _workbench.Clock.Advance(Time.deltaTime);
            double t = _workbench.Clock.TimeSeconds;
            ApplyTraceAt(t);
            SyncOverlays(t);

            if (_smoke)
                DriveSmoke();
        }

        private void HandleInput()
        {
            if (Input.GetKeyDown(KeyCode.Space))
            {
                TogglePlay();
                if (hud != null) hud.Refresh();
            }

            for (int d = 1; d <= CameraDirector.Ids.Count; d++)
            {
                if (Input.GetKeyDown(KeyCode.Alpha0 + d) || Input.GetKeyDown(KeyCode.Keypad0 + d))
                    _workbench.Cameras.SwitchTo(CameraDirector.ForDigit(d));
            }

            if (Input.GetKeyDown(KeyCode.C))
            {
                _workbench.SetCleanView(!_workbench.CleanView);
                if (hud != null) hud.Refresh();
            }
        }

        private void PumpUntilReady()
        {
            if (_runner == null)
                return;
            bool active = _runner.Pump();

            if (_runner.State == RunState.Ready)
            {
                OnTraceReady(_runner.Trace);
                return;
            }
            if (_runner.State == RunState.Failed)
            {
                Fail(_runner.LastError ?? "run failed");
                return;
            }
            if (!active && _runner.State != RunState.Ready)
            {
                Fail($"run ended without a trace (state {_runner.State})");
                return;
            }
            if (Time.realtimeSinceStartup > _deadline)
                Fail("bridge run timed out");
        }

        private void OnTraceReady(FlightTrace trace)
        {
            _workbench = new ReviewWorkbench(trace, FrameRate);
            if (hud != null)
                hud.SetWorkbench(_workbench);

            // Bind the logical camera director to the physical Cinemachine rig so
            // digit-key selection actually blends the live shot.
            var rig = anchors != null && anchors.cameraRig != null
                ? anchors.cameraRig.GetComponent<CameraRig>()
                : null;
            if (rig != null)
                rig.Bind(_workbench.Cameras);

            // Build the engineering overlays from the immutable trace and show the
            // initially-visible set.
            if (layers != null)
            {
                layers.anchors = anchors;
                layers.Initialize(trace);
                layers.ApplyVisibility(_workbench.Snapshot().VisibleLayers);
                _lastLayerRevision = _workbench.LayerRevision;
            }

            // The trace is fully in memory and immutable; the review replays by
            // seeking, so the live bridge is no longer needed. Shut it down now so
            // no child process outlives trace acceptance.
            ShutdownBridge();

            ApplyTraceAt(_workbench.Clock.TimeSeconds);
            SyncOverlays(_workbench.Clock.TimeSeconds);
            SetLabel("trace-hash", $"trace {Short(trace.TraceHash, 16)}…");
            _bound = true;

            if (_smoke)
            {
                // Exercise the exact code path the Space key runs.
                TogglePlay();
                _smokeStartTime = _workbench.Clock.TimeSeconds;
            }

            if (_visual)
            {
                _workbench.Cameras.SwitchTo(_visualCameraId);

                // Advance to a nonzero, powered-ascent time so the vehicle is
                // airborne against the terrain with the plume lit — the honest
                // frame to prove the render.
                var lift = trace.SeekEvent("Liftoff");
                double span = trace.EndSeconds - trace.StartSeconds;
                double defaultTime = lift.HasValue
                    ? Math.Min(trace.EndSeconds, lift.Value + 0.6)
                    : trace.StartSeconds + 0.1 * span;
                _visualTargetTime = _visualRequestedTime ?? defaultTime;
                _visualTargetTime = Math.Max(trace.StartSeconds, Math.Min(trace.EndSeconds, _visualTargetTime));
                Screen.SetResolution(1920, 1080, false);
            }
        }

        private void TogglePlay()
        {
            if (_workbench.Clock.IsPlaying) _workbench.Clock.Pause();
            else _workbench.Clock.Play();
        }

        /// <summary>Drive the vehicle pose and plume from the trace at time <paramref name="t"/>.</summary>
        private void ApplyTraceAt(double t)
        {
            var trace = _workbench.Trace;
            if (anchors != null && anchors.vehicleRoot != null)
            {
                var pos = trace.PositionAt(t);
                if (!pos.IsGap)
                    anchors.vehicleRoot.localPosition = pos.Value;

                // Point the stacked-along-+Y airframe along its velocity heading.
                var vel = trace.VelocityAt(t);
                if (!vel.IsGap && vel.Value.sqrMagnitude > 1e-6f)
                    anchors.vehicleRoot.localRotation =
                        Quaternion.FromToRotation(Vector3.up, vel.Value.normalized);
            }
            if (plume != null)
                plume.ApplyAt(trace.Events, t);
        }

        /// <summary>Re-apply layer visibility on change and refresh trace-time overlays.</summary>
        private void SyncOverlays(double t)
        {
            if (layers == null)
                return;
            if (_workbench.LayerRevision != _lastLayerRevision)
            {
                layers.ApplyVisibility(_workbench.Snapshot().VisibleLayers);
                _lastLayerRevision = _workbench.LayerRevision;
            }
            layers.UpdateAt(t);
        }

        private void DriveSmoke()
        {
            _smokeFrames++;
            if (_smokeFrames < SmokeAdvanceFrames)
                return;
            bool advanced = _workbench.Clock.TimeSeconds > _smokeStartTime + 1e-6;
            bool noChild = _bridge == null || !_bridge.IsRunning;
            WriteSmoke(advanced && noChild, advanced, noChild);
        }

        // --- Visual acceptance gate ---------------------------------------------

        private void DriveVisual()
        {
            _visualFrames++;
            if (_visualFrames == 1)
            {
                // Park the clock at the proof time and paint the scene + panels once.
                _workbench.Clock.Pause();
                _workbench.Clock.Seek(_visualTargetTime);
                ApplyTraceAt(_visualTargetTime);
                SyncOverlays(_visualTargetTime);
                if (hud != null) hud.Refresh();
                return;
            }
            if (_visualFrames < VisualSettleFrames)
                return;
            if (!_visualCapturing)
            {
                _visualCapturing = true;
                StartCoroutine(CaptureAndVerify());
            }
        }

        private IEnumerator CaptureAndVerify()
        {
            yield return new WaitForEndOfFrame();

            int w = Screen.width, h = Screen.height;
            var tex = new Texture2D(w, h, TextureFormat.RGB24, false);
            tex.ReadPixels(new Rect(0, 0, w, h), 0, 0);
            tex.Apply();

            bool shotWritten = false;
            try
            {
                if (!string.IsNullOrEmpty(_visualShot))
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(_visualShot));
                    File.WriteAllBytes(_visualShot, tex.EncodeToPNG());
                    shotWritten = true;
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"review-visual screenshot write failed: {ex.Message}");
            }

            double lum = MeanCentralLuminance(tex);

            // Prove the vehicle and the lit plume are actually being drawn — central
            // luminance alone passed a near-black frame where the metallic airframe
            // collapsed to a silhouette and the plume never appeared. HDRP renders
            // its lighting in Linear; a Gamma project is the failure that produced
            // that black frame, so assert the color space here too.
            var cam = RenderCamera();
            var booster = anchors != null && anchors.boosterStage != null
                ? anchors.boosterStage.GetComponentInChildren<Renderer>() : null;
            var sustainer = anchors != null && anchors.sustainerStage != null
                ? anchors.sustainerStage.GetComponentInChildren<Renderer>() : null;
            var plumeCore = plume != null ? plume.plumeCore : null;

            var boosterProbe = ProbeRenderer("vehicle.booster", booster, cam, tex);
            var sustainerProbe = ProbeRenderer("vehicle.sustainer", sustainer, cam, tex);
            var plumeProbe = ProbeRenderer("plume.core", plumeCore, cam, tex);
            UnityEngine.Object.Destroy(tex);

            bool colorSpaceLinear = QualitySettings.activeColorSpace == ColorSpace.Linear;
            bool vehicleVisible = boosterProbe.Drawn || sustainerProbe.Drawn;
            bool plumeVisible = plumeProbe.Drawn;
            double vehicleLum = Math.Max(boosterProbe.RegionLum, sustainerProbe.RegionLum);
            double plumeLum = plumeProbe.RegionLum;

            Debug.Log($"ASCENT_RENDER_PROBE camera=\"{(cam != null ? cam.name : "null")}\" "
                + $"colorSpaceLinear={colorSpaceLinear}\n  {boosterProbe.Detail}\n  {sustainerProbe.Detail}\n  {plumeProbe.Detail}");

            // Read the live panels back to prove State/Evidence are populated.
            int stateRows = 0, evidenceRows = 0;
            var root = HudRoot();
            if (root != null)
            {
                stateRows = root.Q<VisualElement>("state-readouts")?.childCount ?? 0;
                evidenceRows = root.Q<VisualElement>("evidence-drawer")?.childCount ?? 0;
            }

            bool timeNonzero = _workbench.Clock.TimeSeconds > 1e-6;
            bool targetResolution = w == VisualWidth && h == VisualHeight;
            bool sceneVisible = lum > VisualLuminanceFloor;
            bool ok = timeNonzero && stateRows > 0 && evidenceRows > 0 && sceneVisible
                && shotWritten && targetResolution
                && colorSpaceLinear && vehicleVisible && plumeVisible;
            WriteVisual(ok, timeNonzero, stateRows, evidenceRows, lum, sceneVisible, shotWritten, w, h,
                colorSpaceLinear, vehicleVisible, plumeVisible, vehicleLum, plumeLum);
        }

        // Unity's magenta fallback shader; a renderer wearing it is a stripped/missing
        // shader, never an acceptable render.
        private const string ErrorShaderName = "Hidden/InternalErrorShader";

        /// <summary>The single camera actually rendering the review (carries the brain).</summary>
        private Camera RenderCamera()
        {
            if (anchors != null && anchors.reviewCamera != null)
            {
                var c = anchors.reviewCamera.GetComponent<Camera>();
                if (c != null)
                    return c;
            }
            return Camera.main ?? (Camera.allCameras.Length > 0 ? Camera.allCameras[0] : null);
        }

        /// <summary>The result of asserting one renderer is actually being drawn.</summary>
        private struct RenderProbe
        {
            public bool Drawn;       // enabled + non-error shader + in culling mask + in frustum
            public double RegionLum; // mean luminance under the object's screen-space bounds
            public string Detail;    // full breakdown, for the diagnostic log
        }

        /// <summary>
        /// Proves a renderer is set up to appear on camera: it is enabled and active,
        /// wears a real (non-magenta) shader, sits in the render camera's culling mask,
        /// and is inside the frustum this frame. Also measures the framebuffer luminance
        /// under its projected bounds so an all-black draw is visible in the log. This is
        /// what keeps the gate from passing on an invisible vehicle or plume.
        /// </summary>
        private static RenderProbe ProbeRenderer(string label, Renderer r, Camera cam, Texture2D tex)
        {
            var p = new RenderProbe();
            if (r == null)
            {
                p.Detail = $"{label}: renderer=null";
                return p;
            }
            bool enabled = r.enabled && r.gameObject.activeInHierarchy;
            var shader = r.sharedMaterial != null ? r.sharedMaterial.shader : null;
            bool goodShader = shader != null && shader.name != ErrorShaderName;
            bool inMask = cam != null && (cam.cullingMask & (1 << r.gameObject.layer)) != 0;
            bool inFrustum = r.isVisible;
            p.RegionLum = cam != null ? RegionLuminance(r.bounds, cam, tex) : 0.0;
            p.Drawn = enabled && goodShader && inMask && inFrustum;
            p.Detail = $"{label}: enabled={enabled} shader=\"{(shader != null ? shader.name : "null")}\" "
                     + $"inMask={inMask} inFrustum={inFrustum} regionLum={p.RegionLum:0.###}";
            return p;
        }

        /// <summary>Mean luminance of the framebuffer under a world-space bounds, projected
        /// to screen. Zero if the bounds fall entirely behind or off the camera.</summary>
        private static double RegionLuminance(Bounds b, Camera cam, Texture2D tex)
        {
            Vector3 min = b.min, max = b.max;
            float sxMin = float.MaxValue, sxMax = float.MinValue, syMin = float.MaxValue, syMax = float.MinValue;
            bool anyFront = false;
            for (int i = 0; i < 8; i++)
            {
                var corner = new Vector3((i & 1) == 0 ? min.x : max.x,
                                         (i & 2) == 0 ? min.y : max.y,
                                         (i & 4) == 0 ? min.z : max.z);
                var sp = cam.WorldToScreenPoint(corner);
                if (sp.z <= 0f)
                    continue; // behind the camera
                anyFront = true;
                sxMin = Mathf.Min(sxMin, sp.x); sxMax = Mathf.Max(sxMax, sp.x);
                syMin = Mathf.Min(syMin, sp.y); syMax = Mathf.Max(syMax, sp.y);
            }
            if (!anyFront)
                return 0.0;
            int x0 = Mathf.Clamp(Mathf.FloorToInt(sxMin), 0, tex.width - 1);
            int x1 = Mathf.Clamp(Mathf.CeilToInt(sxMax), 0, tex.width - 1);
            int y0 = Mathf.Clamp(Mathf.FloorToInt(syMin), 0, tex.height - 1);
            int y1 = Mathf.Clamp(Mathf.CeilToInt(syMax), 0, tex.height - 1);
            int w = x1 - x0 + 1, h = y1 - y0 + 1;
            if (w <= 0 || h <= 0)
                return 0.0;
            var px = tex.GetPixels(x0, y0, w, h);
            if (px.Length == 0)
                return 0.0;
            double sum = 0.0;
            foreach (var c in px)
                sum += 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
            return sum / px.Length;
        }

        /// <summary>Mean relative luminance of the central half of the frame — the
        /// viewport hole, away from the edge panels. Near zero for a black viewport.</summary>
        private static double MeanCentralLuminance(Texture2D tex)
        {
            int w = tex.width, h = tex.height;
            int x0 = w / 4, y0 = h / 4;
            int cw = w / 2, ch = h / 2;
            if (cw <= 0 || ch <= 0)
                return 0.0;
            var px = tex.GetPixels(x0, y0, cw, ch);
            if (px.Length == 0)
                return 0.0;
            double sum = 0.0;
            foreach (var c in px)
                sum += 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
            return sum / px.Length;
        }

        private void WriteVisual(bool ok, bool timeNonzero, int stateRows, int evidenceRows, double lum,
            bool sceneVisible, bool shotWritten, int width, int height,
            bool colorSpaceLinear, bool vehicleVisible, bool plumeVisible, double vehicleLum, double plumeLum)
        {
            _visualDone = true;
            string hash = _workbench?.Trace?.TraceHash ?? "";
            double t = _workbench?.Clock.TimeSeconds ?? 0.0;
            string json = "{\n"
                + "  \"kind\": \"review-visual\",\n"
                + $"  \"mission_id\": \"{MissionId}\",\n"
                + $"  \"trace_sha256\": \"{hash}\",\n"
                + $"  \"review_time_s\": {Num(t)},\n"
                + $"  \"time_nonzero\": {Bool(timeNonzero)},\n"
                + $"  \"state_rows\": {stateRows},\n"
                + $"  \"evidence_rows\": {evidenceRows},\n"
                + $"  \"scene_mean_luminance\": {Num(lum)},\n"
                + $"  \"scene_visible\": {Bool(sceneVisible)},\n"
                + $"  \"color_space_linear\": {Bool(colorSpaceLinear)},\n"
                + $"  \"vehicle_visible\": {Bool(vehicleVisible)},\n"
                + $"  \"vehicle_region_luminance\": {Num(vehicleLum)},\n"
                + $"  \"plume_visible\": {Bool(plumeVisible)},\n"
                + $"  \"plume_region_luminance\": {Num(plumeLum)},\n"
                + $"  \"screenshot\": {JsonStr(_visualShot)},\n"
                + $"  \"screenshot_written\": {Bool(shotWritten)},\n"
                + $"  \"resolution\": \"{width}x{height}\",\n"
                + $"  \"target_resolution\": \"{VisualWidth}x{VisualHeight}\",\n"
                + $"  \"target_resolution_met\": {Bool(width == VisualWidth && height == VisualHeight)},\n"
                + $"  \"ok\": {Bool(ok)}\n"
                + "}\n";
            try
            {
                if (!string.IsNullOrEmpty(_visualOut))
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(_visualOut));
                    File.WriteAllText(_visualOut, json);
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"review-visual write failed: {ex.Message}");
            }

            if (ok)
                Debug.Log($"ASCENT_REVIEW_VISUAL_OK t={t:0.###} lum={lum:0.###} state={stateRows} evidence={evidenceRows} vehicle={vehicleVisible} plume={plumeVisible} shot={_visualShot}");
            else
                Debug.LogError($"ASCENT_REVIEW_VISUAL_FAIL t={t:0.###} nonzero={timeNonzero} state={stateRows} evidence={evidenceRows} lum={lum:0.###} visible={sceneVisible} linear={colorSpaceLinear} vehicle={vehicleVisible} plume={plumeVisible} shot={shotWritten}");

            Application.Quit(ok ? 0 : 8);
        }

        private VisualElement HudRoot()
        {
            var doc = hud != null ? hud.GetComponent<UIDocument>() : null;
            return doc != null ? doc.rootVisualElement : null;
        }

        private void Fail(string reason)
        {
            _failed = true;
            Debug.LogError($"ReviewBootstrap: {reason}");
            SetLabel("status", "Bridge error");
            SetLabel("trace-hash", reason);
            ShutdownBridge();
            if (_smoke && !_smokeDone)
                WriteSmoke(false, false, _bridge == null || !_bridge.IsRunning);
            if (_visual && !_visualDone)
                WriteVisual(false, false, 0, 0, 0.0, false, false, Screen.width, Screen.height,
                    false, false, false, 0.0, 0.0);
        }

        private void WriteSmoke(bool ok, bool advanced, bool noChild)
        {
            _smokeDone = true;
            string hash = _workbench?.Trace?.TraceHash ?? "";
            string json = "{\n"
                + "  \"kind\": \"review-smoke\",\n"
                + $"  \"mission_id\": \"{MissionId}\",\n"
                + $"  \"trace_sha256\": \"{hash}\",\n"
                + $"  \"trace_hash_len\": {hash.Length},\n"
                + $"  \"hud_ready\": {Bool(_bound)},\n"
                + $"  \"space_advances_playback\": {Bool(advanced)},\n"
                + $"  \"clean_shutdown_no_child\": {Bool(noChild)},\n"
                + $"  \"ok\": {Bool(ok)}\n"
                + "}\n";
            try
            {
                if (!string.IsNullOrEmpty(_smokeOut))
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(_smokeOut));
                    File.WriteAllText(_smokeOut, json);
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"review-smoke write failed: {ex.Message}");
            }

            if (ok)
                Debug.Log($"ASCENT_REVIEW_SMOKE_OK trace={Short(hash, 12)} advanced={advanced} noChild={noChild} out={_smokeOut}");
            else
                Debug.LogError($"ASCENT_REVIEW_SMOKE_FAIL ready={_bound} advanced={advanced} noChild={noChild}");

            Application.Quit(ok ? 0 : 7);
        }

        private void ShutdownBridge()
        {
            try { _bridge?.Shutdown(); } catch (Exception) { }
        }

        private void OnDestroy() => ShutdownBridge();

        private void ParseArgs()
        {
            var args = Environment.GetCommandLineArgs();
            for (int i = 0; i < args.Length; i++)
            {
                if (args[i] == "-ascent-review-smoke")
                    _smoke = true;
                else if (args[i] == "-ascent-review-visual")
                    _visual = true;
                else if (args[i] == "-ascent-out" && i + 1 < args.Length)
                    _smokeOut = _visualOut = args[i + 1];
                else if (args[i] == "-ascent-shot" && i + 1 < args.Length)
                    _visualShot = args[i + 1];
                else if (args[i] == "-ascent-review-camera" && i + 1 < args.Length)
                    _visualCameraId = args[i + 1];
                else if (args[i] == "-ascent-review-time" && i + 1 < args.Length
                         && double.TryParse(args[i + 1], NumberStyles.Float, CultureInfo.InvariantCulture,
                             out double requestedTime))
                    _visualRequestedTime = requestedTime;
            }
            if (_smoke && string.IsNullOrEmpty(_smokeOut))
                _smokeOut = Path.Combine(Application.persistentDataPath, "review-smoke.json");
            if (_visual)
            {
                if (string.IsNullOrEmpty(_visualOut))
                    _visualOut = Path.Combine(Application.persistentDataPath, "review-visual.json");
                if (string.IsNullOrEmpty(_visualShot))
                    _visualShot = Path.Combine(Application.persistentDataPath, "review-visual.png");
            }
        }

        private void SetLabel(string name, string text)
        {
            var root = HudRoot();
            var label = root?.Q<Label>(name);
            if (label != null)
                label.text = text;
        }

        private static string Bool(bool b) => b ? "true" : "false";

        private static string Num(double v) => v.ToString("0.######", CultureInfo.InvariantCulture);

        private static string JsonStr(string s) =>
            "\"" + (s ?? string.Empty).Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";

        private static string Short(string s, int n) =>
            string.IsNullOrEmpty(s) ? "" : s.Substring(0, Math.Min(n, s.Length));
    }
}
