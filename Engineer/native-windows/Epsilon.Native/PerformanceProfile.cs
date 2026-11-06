using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Media;

namespace Epsilon.Native;

// Opt-in measurement only. Normal launches keep their existing behavior.
internal static class PerformanceProfile
{
    public static async Task RunAsync(PanelWindow window, string output)
    {
        window.WindowStyle = WindowStyle.None;
        window.ResizeMode = ResizeMode.NoResize;
        window.Width = 1000; window.Height = 700; window.Left = 40; window.Top = 40;
        window.FrameCounter.Stop(); window.FrameCounter.Visibility = Visibility.Collapsed;
        await Task.Delay(6000);
        // Warm the interaction paths and allow tiered compilation to settle.
        for (int i = 1; i <= 30; i++) { window.SwitchProfile(); if (i % 7 == 0) window.ResetProfile(); await Task.Delay(100); }
        for (int i = 1; i <= 60; i++) { window.Controller.Set("reactor", (int)Math.Round((.5 + .48 * Math.Sin(i * Math.PI / 30)) * 300)); await Task.Delay(33); }
        window.Controller.Set("reactor", 100); window.ResetProfile();
        await Task.Delay(6000);
        using var process = Process.GetCurrentProcess();
        var phases = new List<object>();
        foreach (string phase in new[] { "idle", "switching", "settled", "dragging" })
        {
            int count = phase == "switching" ? 120 : phase == "dragging" ? 300 : 0;
            int duration = phase == "switching" ? 12000 : phase == "dragging" ? 10000 : 5000;
            process.Refresh(); double cpu = process.TotalProcessorTime.TotalMilliseconds;
            var start = DateTimeOffset.UtcNow; var clock = Stopwatch.StartNew();
            int actions = 0, resets = 0;
            if (count == 0) await Task.Delay(duration);
            else for (int i = 1; i <= count; i++)
            {
                await Task.Delay(Math.Max(1, (int)(i * duration / (double)count - clock.Elapsed.TotalMilliseconds)));
                if (phase == "switching") { window.SwitchProfile(); if (i % 7 == 0) { window.ResetProfile(); resets++; } }
                else window.Controller.Set("reactor", (int)Math.Round((.5 + .48 * Math.Sin(i * Math.PI / 30)) * 300));
                actions++;
            }
            var end = DateTimeOffset.UtcNow; process.Refresh();
            double elapsed = clock.Elapsed.TotalMilliseconds;
            phases.Add(new { name = phase, start, end, elapsedMs = elapsed, actions, resets,
                cpuMs = process.TotalProcessorTime.TotalMilliseconds - cpu, privateMiB = process.PrivateMemorySize64 / 1048576.0,
                pids = new[] { new { id = Environment.ProcessId, role = "Native WPF" } } });
        }
        var report = new { implementation = "native", timestamp = DateTimeOffset.UtcNow, width = window.ActualWidth, height = window.ActualHeight,
            deviceScaleFactor = VisualTreeHelper.GetDpi(window).DpiScaleX, reducedMotion = window.ReducedMotion, renderTier = RenderCapability.Tier >> 16,
            logicalProcessors = Environment.ProcessorCount, runtime = Environment.Version.ToString(), overlayEnabled = false, interactionWarmup = true, phases };
        Directory.CreateDirectory(Path.GetDirectoryName(output)!);
        await File.WriteAllTextAsync(output, JsonSerializer.Serialize(report, new JsonSerializerOptions { WriteIndented = true }));
        window.Close();
    }
}
