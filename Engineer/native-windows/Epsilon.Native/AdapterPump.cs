using System.Threading.Channels;
using Epsilon.Core;

namespace Epsilon.Native;

// Latest-wins bounded queue: no I/O, waiting or unbounded task creation in an input handler.
public sealed class AdapterPump : IDisposable
{
    readonly Channel<ControllerSnapshot> queue = Channel.CreateBounded<ControllerSnapshot>(new BoundedChannelOptions(1) { FullMode = BoundedChannelFullMode.DropOldest, SingleReader = true });
    readonly CancellationTokenSource stop = new();
    readonly Task worker;
    public AdapterPump(IControllerAdapter adapter, Action<Exception> onError)
    {
        worker = Task.Run(async () =>
        {
            try
            {
                await foreach (var snapshot in queue.Reader.ReadAllAsync(stop.Token))
                    try { await adapter.ApplyAsync(snapshot, stop.Token); }
                    catch (OperationCanceledException) when (stop.IsCancellationRequested) { break; }
                    catch (Exception error) { onError(error); }
            }
            catch (OperationCanceledException) { }
        });
    }
    public void Send(ControllerSnapshot snapshot) => queue.Writer.TryWrite(snapshot);
    public void Dispose() { queue.Writer.TryComplete(); stop.Cancel(); _ = worker.ContinueWith(_ => stop.Dispose(), TaskScheduler.Default); }
}
