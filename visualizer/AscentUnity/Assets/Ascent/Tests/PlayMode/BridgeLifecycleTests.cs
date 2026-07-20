using System.Collections;
using System.Diagnostics;
using System.IO;
using Ascent.Runtime.Bridge;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

namespace Ascent.Tests.PlayMode
{
    /// <summary>
    /// Drives the real, Cargo-built <c>ascent-visualizer-bridge</c> binary (never a
    /// line-oriented mock): negotiation, a forced child exit with one restart, and
    /// a graceful quit that leaves no orphan process.
    /// </summary>
    public class BridgeLifecycleTests
    {
        // Application.dataPath = <repo>/visualizer/AscentUnity/Assets
        private static string BridgePath =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../../../target/debug/ascent-visualizer-bridge"));

        private static void AssertBinaryPresent()
        {
            if (!File.Exists(BridgePath))
                Assert.Ignore($"bridge binary missing at {BridgePath}; run `cargo build -p ascent-visualizer-bridge`");
        }

        [UnityTest]
        public IEnumerator SuccessfulNegotiationYieldsHelloAck()
        {
            AssertBinaryPresent();
            var bridge = new BridgeProcess(BridgePath);
            bridge.Start();
            bridge.Send("c-hello", ClientRequest.Hello("unity-test"));

            Envelope ack = null;
            float deadline = Time.realtimeSinceStartup + 5f;
            while (ack == null && Time.realtimeSinceStartup < deadline)
            {
                if (bridge.Poll(out var msg))
                    ack = msg.Envelope;
                yield return null;
            }
            Assert.That(ack, Is.Not.Null, "no hello_ack within 5s");
            Assert.That(ack.Kind, Is.EqualTo("hello_ack"));
            bridge.Shutdown();
        }

        [UnityTest]
        public IEnumerator ForcedChildExitTriggersOneRestart()
        {
            AssertBinaryPresent();
            var bridge = new BridgeProcess(BridgePath, maxRestarts: 1);
            bridge.Start();
            int gen0 = bridge.Generation;
            int? pid0 = bridge.ChildPid;
            Assert.That(pid0, Is.Not.Null);

            // Force an unexpected exit by killing the child out from under us.
            Process.GetProcessById(pid0.Value).Kill();

            float deadline = Time.realtimeSinceStartup + 5f;
            while (bridge.Generation == gen0 && Time.realtimeSinceStartup < deadline)
                yield return null;

            Assert.That(bridge.Generation, Is.GreaterThan(gen0), "no restart after forced exit");
            Assert.That(bridge.IsRunning, Is.True, "restarted process not running");
            Assert.That(bridge.ChildPid, Is.Not.EqualTo(pid0), "restart reused the dead pid");
            bridge.Shutdown();
        }

        [UnityTest]
        public IEnumerator GracefulQuitLeavesNoOrphan()
        {
            AssertBinaryPresent();
            var bridge = new BridgeProcess(BridgePath);
            bridge.Start();
            int? pid = bridge.ChildPid;
            Assert.That(pid, Is.Not.Null);
            bridge.Shutdown();

            Assert.That(bridge.IsRunning, Is.False);
            Assert.That(bridge.ChildPid, Is.Null);

            // No orphan: the pid must either be gone (reaped) or no longer our
            // bridge image (pid reuse). Allow the OS a moment to reap.
            bool orphan = true;
            float deadline = Time.realtimeSinceStartup + 3f;
            while (orphan && Time.realtimeSinceStartup < deadline)
            {
                orphan = IsLiveBridge(pid.Value);
                if (orphan)
                    yield return null;
            }
            Assert.That(orphan, Is.False, "bridge process still alive after shutdown");
        }

        private static bool IsLiveBridge(int pid)
        {
            try
            {
                var p = Process.GetProcessById(pid);
                if (p.HasExited)
                    return false;
                return p.ProcessName.Contains("ascent-visualizer-bridge");
            }
            catch
            {
                return false; // no such process — reaped
            }
        }
    }
}
