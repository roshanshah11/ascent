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
            try
            {
                while (!token.IsCancellationRequested)
                {
                    int n = stdout.Read(buffer, 0, buffer.Length);
                    if (n <= 0)
                        break;
                    var span = new ReadOnlySpan<byte>(buffer, 0, n);
                    foreach (var env in decoder.Push(span))
                        _inbound.Enqueue(new BridgeMessage(generation, env));
                }
            }
            catch (Exception)
            {
                // Fall through to exit handling below.
            }

            if (token.IsCancellationRequested || _disposed)
                return;
            // Unexpected end of stream: signal, then attempt one restart.
            UnexpectedExit?.Invoke(generation);
            TryRestart(generation);
        }

        private static void DrainStderr(Stream stderr, CancellationToken token)
        {
            var buffer = new byte[4096];
            try
            {
                while (!token.IsCancellationRequested && stderr.Read(buffer, 0, buffer.Length) > 0)
                {
                    // Diagnostic only; never parsed as protocol.
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
