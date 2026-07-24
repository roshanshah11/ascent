using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;

namespace Ascent.Runtime.Bridge
{
    /// <summary>A message published on the main thread, tagged with its process generation.</summary>
    public readonly struct BridgeMessage
    {
        public readonly int Generation;
        public readonly Envelope Envelope;

        public BridgeMessage(int generation, Envelope envelope)
        {
            Generation = generation;
            Envelope = envelope;
        }
    }

    /// <summary>
    /// Supervises the read-only Rust bridge as a child process over binary framed
    /// stdio. Runs the reader off the main thread and publishes immutable messages
    /// through a queue drained by <see cref="Poll"/>. On one unexpected exit it
    /// restarts once with a new generation; messages from an earlier generation
    /// are discarded. Application quit always shuts down and reaps the child.
    ///
    /// Never uses ReadLine/OutputDataReceived or newline-delimited JSON.
    /// </summary>
    public sealed class BridgeProcess : IDisposable
    {
        // BRIDGE-DIAG (removable): flips on the launch/frame/exit tracing used to
        // diagnose the packaged bridge-timeout. Set false to silence.
        private const bool Diag = true;

        private readonly string _executablePath;
        private readonly int _maxRestarts;
        private readonly ConcurrentQueue<BridgeMessage> _inbound = new ConcurrentQueue<BridgeMessage>();

        private Process _process;
        private Task _readerTask;
        private CancellationTokenSource _readerCts;
        private int _generation;
        private int _restartsUsed;
        private volatile bool _disposed;

        public BridgeProcess(string executablePath, int maxRestarts = 1)
        {
            _executablePath = executablePath;
            _maxRestarts = maxRestarts;
        }

        public int Generation => _generation;

        public bool IsRunning
        {
            get
            {
                var p = _process;
                if (p == null)
                    return false;
                try { return !p.HasExited; }
                catch { return false; }
            }
        }

        public int? ChildPid
        {
            get
            {
                var p = _process;
                if (p == null)
                    return null;
                try { return p.HasExited ? (int?)null : p.Id; }
                catch { return null; }
            }
        }

        /// <summary>Occurs (on the reader thread) when the child exits unexpectedly.</summary>
        public event Action<int> UnexpectedExit;

        public void Start()
        {
            var psi = new ProcessStartInfo
            {
                FileName = _executablePath,
                Arguments = "--stdio",
                UseShellExecute = false,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true,
            };
            var proc = new Process { StartInfo = psi, EnableRaisingEvents = false };
            proc.Start();

            // BRIDGE-DIAG (removable): record exactly what was launched.
            if (Diag)
            {
                int pid = -1;
                try { pid = proc.Id; } catch { }
                UnityEngine.Debug.Log($"BRIDGE-DIAG launch: exe=\"{_executablePath}\" args=\"{psi.Arguments}\" pid={pid}");
            }

            var cts = new CancellationTokenSource();
            var stdout = proc.StandardOutput.BaseStream;
            var stderr = proc.StandardError.BaseStream;
            var token = cts.Token;

            // Publish the fully-started process and its generation together, so a
            // concurrent reader of IsRunning/Generation never sees a half-built one.
            _process = proc;
            _readerCts = cts;
            int generation = System.Threading.Interlocked.Increment(ref _generation);

            // Drain stderr independently so it can never back-pressure stdout.
            _ = Task.Run(() => DrainStderr(stderr, token));
            _readerTask = Task.Run(() => ReadFrames(stdout, generation, token));
        }

        private void ReadFrames(Stream stdout, int generation, CancellationToken token)
        {
            var decoder = new FrameDecoder(Protocol.MaxFrameBytes);
            var buffer = new byte[8192];
            long totalBytes = 0;   // BRIDGE-DIAG
            int frameCount = 0;    // BRIDGE-DIAG
            bool firstRead = true; // BRIDGE-DIAG
            try
            {
                while (!token.IsCancellationRequested)
                {
                    int n = stdout.Read(buffer, 0, buffer.Length);
                    if (n <= 0)
                        break;
                    // BRIDGE-DIAG (removable): show the head of the very first read as
                    // hex — a valid stream starts with a 4-byte little-endian length,
                    // whereas stray stdout text would show up as ASCII here.
                    if (Diag && firstRead)
                    {
                        firstRead = false;
                        UnityEngine.Debug.Log($"BRIDGE-DIAG first stdout read: {n} bytes, head={HexHead(buffer, n, 48)}");
                    }
                    totalBytes += n;
                    var span = new ReadOnlySpan<byte>(buffer, 0, n);
                    foreach (var env in decoder.Push(span))
                    {
                        _inbound.Enqueue(new BridgeMessage(generation, env));
                        if (Diag)
                        {
                            frameCount++;
                            UnityEngine.Debug.Log($"BRIDGE-DIAG frame #{frameCount}: kind=\"{env.Kind}\" pv={env.ProtocolVersion}");
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                // A decode/read failure otherwise looks identical to a silently
                // stalled child, so surface it rather than swallowing it.
                if (!token.IsCancellationRequested && !_disposed)
                    UnityEngine.Debug.LogError($"BridgeProcess: frame reader failed (gen {generation}): {ex}");
            }

            // BRIDGE-DIAG (removable): the reader loop only ends on EOF, cancel, or
            // throw — record why, plus the child's exit code if it has exited.
            if (Diag)
            {
                string exit = "still-running";
                try { if (_process != null && _process.HasExited) exit = _process.ExitCode.ToString(); }
                catch { exit = "unknown"; }
                UnityEngine.Debug.Log($"BRIDGE-DIAG reader ended (gen {generation}): totalBytes={totalBytes} frames={frameCount} poisoned={decoder.IsPoisoned} cancelled={token.IsCancellationRequested} childExit={exit}");
            }

            if (token.IsCancellationRequested || _disposed)
                return;
            // Unexpected end of stream: signal, then attempt one restart.
            UnexpectedExit?.Invoke(generation);
            TryRestart(generation);
        }

        // BRIDGE-DIAG (removable): hex + ASCII of the first bytes off stdout.
        private static string HexHead(byte[] buffer, int n, int max)
        {
            int count = System.Math.Min(n, max);
            var sb = new System.Text.StringBuilder();
            for (int i = 0; i < count; i++)
                sb.Append(buffer[i].ToString("x2")).Append(' ');
            sb.Append("| ");
            for (int i = 0; i < count; i++)
            {
                byte b = buffer[i];
                sb.Append(b >= 0x20 && b < 0x7f ? (char)b : '.');
            }
            return sb.ToString();
        }

        private static void DrainStderr(Stream stderr, CancellationToken token)
        {
            var buffer = new byte[4096];
            try
            {
                int n;
                while (!token.IsCancellationRequested && (n = stderr.Read(buffer, 0, buffer.Length)) > 0)
                {
                    // Never parsed as protocol, but the child writes its own launch
                    // and mission errors here — surface them so a bridge that fails
                    // to produce a trace is not mistaken for a stalled child.
                    var text = System.Text.Encoding.UTF8.GetString(buffer, 0, n).TrimEnd();
                    if (text.Length > 0)
                        UnityEngine.Debug.LogWarning($"[bridge stderr] {text}");
                }
            }
            catch (Exception)
            {
            }
        }

        private void TryRestart(int generation)
        {
            if (_disposed || generation != _generation)
                return;
            if (_restartsUsed >= _maxRestarts)
                return;
            _restartsUsed++;
            try
            {
                ReapCurrent();
                Start();
            }
            catch (Exception)
            {
                // A failed restart leaves the bridge stopped; callers surface it.
            }
        }

        /// <summary>Send a client request payload to the child as one frame.</summary>
        public void Send(string messageId, JObject payload, string requestId = null)
        {
            if (!IsRunning)
                throw new InvalidOperationException("bridge is not running");
            var envelope = new JObject
            {
                ["protocol_version"] = Protocol.Version,
                ["message_id"] = messageId,
                ["kind"] = (string)payload["kind"],
                ["payload"] = payload,
            };
            if (requestId != null)
                envelope["request_id"] = requestId;
            var frame = FrameDecoder.Encode(envelope);
            var stdin = _process.StandardInput.BaseStream;
            stdin.Write(frame, 0, frame.Length);
            stdin.Flush();
        }

        /// <summary>Drain queued messages from the current generation; earlier ones are dropped.</summary>
        public bool Poll(out BridgeMessage message)
        {
            while (_inbound.TryDequeue(out var candidate))
            {
                if (candidate.Generation == _generation)
                {
                    message = candidate;
                    return true;
                }
                // Stale generation — discard.
            }
            message = default;
            return false;
        }

        /// <summary>Request clean shutdown, waiting up to two seconds before killing.</summary>
        public void Shutdown()
        {
            if (_process == null)
                return;
            try
            {
                if (!_process.HasExited)
                {
                    Send(Guid.NewGuid().ToString(), ClientRequest.Shutdown());
                    if (!_process.WaitForExit(2000))
                        _process.Kill();
                }
            }
            catch (Exception)
            {
                try { if (!_process.HasExited) _process.Kill(); } catch { }
            }
            finally
            {
                ReapCurrent();
            }
        }

        private void ReapCurrent()
        {
            try { _readerCts?.Cancel(); } catch { }
            try
            {
                if (_process != null && !_process.HasExited)
                {
                    _process.Kill();
                    _process.WaitForExit(1000);
                }
            }
            catch (Exception)
            {
            }
            finally
            {
                _process?.Dispose();
                _process = null;
            }
        }

        public void Dispose()
        {
            if (_disposed)
                return;
            _disposed = true;
            Shutdown();
        }
    }
}
