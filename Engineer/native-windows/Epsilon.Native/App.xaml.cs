using System.IO;
using System.Windows;
using Epsilon.Core;

namespace Epsilon.Native;
public partial class App : Application
{
    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ShutdownMode = ShutdownMode.OnExplicitShutdown;
        string path = Path.Combine(AppContext.BaseDirectory, "config.json");
        int configIndex = Array.IndexOf(e.Args, "--config");
        if (configIndex >= 0 && configIndex + 1 < e.Args.Length) path = Path.GetFullPath(e.Args[configIndex + 1]);
        try
        {
            var configuration = await Task.Run(async () => Configuration.Parse(await File.ReadAllTextAsync(path)));
            var window = new PanelWindow(new Controller(configuration), e.Args.Contains("--windowed"));
            MainWindow = window; ShutdownMode = ShutdownMode.OnMainWindowClose; window.Show();
            int profileIndex = Array.IndexOf(e.Args, "--profile");
            if (profileIndex >= 0 && profileIndex + 1 < e.Args.Length)
                await PerformanceProfile.RunAsync(window, Path.GetFullPath(e.Args[profileIndex + 1]));
            int benchmarkIndex = Array.IndexOf(e.Args, "--benchmark");
            if (benchmarkIndex >= 0 && benchmarkIndex + 1 < e.Args.Length)
                await Diagnostics.BenchmarkAsync(window, Path.GetFullPath(e.Args[benchmarkIndex + 1]));
            int verifyIndex = Array.IndexOf(e.Args, "--verify");
            if (verifyIndex >= 0 && verifyIndex + 1 < e.Args.Length)
                await Diagnostics.VerifyAsync(window, Path.GetFullPath(e.Args[verifyIndex + 1]));
        }
        catch (Exception error)
        {
            int diagnosticIndex = Array.FindIndex(e.Args, a => a is "--verify" or "--benchmark" or "--profile");
            if (diagnosticIndex >= 0 && diagnosticIndex + 1 < e.Args.Length)
            {
                await File.WriteAllTextAsync(e.Args[diagnosticIndex + 1], error.ToString());
                Shutdown(1); return;
            }
            MessageBox.Show($"Unable to start Epsilon Engineering.\n\nConfiguration: {path}\n\n{error.Message}\n\nEdit config.json beside the executable and launch again.",
                "Epsilon Engineering — startup error", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
        }
    }
}
