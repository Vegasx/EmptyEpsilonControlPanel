using Microsoft.Diagnostics.Tracing;
using Microsoft.Diagnostics.Tracing.Etlx;
using Microsoft.Diagnostics.Tracing.Parsers;
using Microsoft.Diagnostics.Tracing.Parsers.Kernel;
using Microsoft.Diagnostics.Tracing.Session;
using System.Text.Json;

var json = new JsonSerializerOptions { WriteIndented = true };
if (args[0] == "capture")
{
    string file = Path.GetFullPath(args[1]);
    try
    {
        if (TraceEventSession.IsElevated() != true) throw new Exception("Administrator permission is required for kernel CPU sampling.");
        using (var session = new TraceEventSession("EpsilonProfile-" + Environment.ProcessId, file))
        {
            session.BufferSizeMB = 128;
            session.EnableKernelProvider(KernelTraceEventParser.Keywords.Profile | KernelTraceEventParser.Keywords.Process |
                KernelTraceEventParser.Keywords.Thread | KernelTraceEventParser.Keywords.ImageLoad, KernelTraceEventParser.Keywords.Profile);
            session.EnableProvider(ClrTraceEventParser.ProviderGuid, TraceEventLevel.Verbose,
                (ulong)(ClrTraceEventParser.Keywords.Jit | ClrTraceEventParser.Keywords.Loader));
            File.WriteAllText(file + ".ready", DateTimeOffset.UtcNow.ToString("O"));
            var deadline = DateTime.UtcNow.AddMinutes(6);
            while (!File.Exists(file + ".stop") && DateTime.UtcNow < deadline) Thread.Sleep(200);
        }
        File.WriteAllText(file + ".done", "stopped");
    }
    catch (Exception error) { File.WriteAllText(file + ".error", error.ToString()); Environment.ExitCode = 1; }
    return;
}
if (args[0] != "analyze") throw new Exception("Expected capture or analyze");
using var log = new TraceLog(TraceLog.CreateFromEventTraceLogFile(args[1]));
var runs = args.Skip(3).Select(path => JsonDocument.Parse(File.ReadAllText(path)).RootElement.Clone()).ToArray();
var results = new List<object>();
foreach (var run in runs)
foreach (var phase in run.GetProperty("phases").EnumerateArray())
{
    var start = phase.GetProperty("start").GetDateTimeOffset().UtcDateTime;
    var end = phase.GetProperty("end").GetDateTimeOffset().UtcDateTime;
    var pids = phase.GetProperty("pids").EnumerateArray().ToDictionary(p => p.GetProperty("id").GetInt32(), p => p.GetProperty("role").GetString()!);
    var buckets = new Dictionary<(string role, string module, string method), int>();
    int total = 0, withStack = 0, excludedInterruptSamples = 0;
    foreach (var e in log.Events.FilterByTime(start, end))
    {
        if (e is not SampledProfileTraceData sample || !pids.TryGetValue(sample.ProcessID, out var role)) continue;
        if (sample.NonProcess) { excludedInterruptSamples++; continue; }
        total++;
        var address = sample.CallStack()?.CodeAddress;
        var module = address?.ModuleName;
        if (string.IsNullOrEmpty(module)) module = sample.Process()?.LoadedModules.GetModuleContainingAddress(sample.InstructionPointer, sample.TimeStampRelativeMSec)?.ModuleFile.Name;
        var method = address?.FullMethodName;
        if (string.IsNullOrEmpty(module)) module = "Unresolved / JIT code";
        if (string.IsNullOrEmpty(method)) method = module;
        if (sample.CallStack() != null) withStack++;
        var key = (role, module, method);
        buckets[key] = buckets.GetValueOrDefault(key) + 1;
    }
    results.Add(new { implementation = run.GetProperty("implementation").GetString(), phase = phase.GetProperty("name").GetString(), totalSamples = total,
        samplesWithStack = withStack, excludedInterruptSamples, sampleIntervalMs = log.SampleProfileInterval.TotalMilliseconds,
        leaves = buckets.OrderByDescending(p => p.Value).Select(p => new { role = p.Key.role, module = p.Key.module, method = p.Key.method, samples = p.Value }).ToArray() });
}
File.WriteAllText(args[2], JsonSerializer.Serialize(new { eventsLost = log.EventsLost, results }, json));
Console.WriteLine("Saved " + args[2]);
