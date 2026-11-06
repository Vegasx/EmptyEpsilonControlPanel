using System.Diagnostics;
using System.Globalization;
using System.Windows;
using System.Windows.Media;

namespace Epsilon.Native;

// Temporary overlay. Counts unique WPF Rendering callbacks, NOT GPU presents.
// Sampling is bounded to interaction/animation bursts so the counter does not
// keep WPF rendering indefinitely when the controller is idle.
internal sealed class FpsCounter : FrameworkElement, IDisposable
{
    static readonly Typeface Typeface = new("Consolas");
    static readonly Brush Ink = CreateInk();
    static readonly long SampleTicks = (long)(Stopwatch.Frequency * .25);
    static readonly long ActivityTailTicks = (long)(Stopwatch.Frequency * .4);
    FormattedText text = null!;
    string caption = "FPS —";
    TimeSpan? lastRenderingTime;
    long activeUntil, sampleStarted;
    int intervals;
    bool disposed;
    internal bool IsSampling { get; private set; }
    internal double? LastFramesPerSecond { get; private set; }
    internal int SamplesPublished { get; private set; }

    public FpsCounter()
    {
        Width = 124; Height = 28;
        HorizontalAlignment = HorizontalAlignment.Right;
        VerticalAlignment = VerticalAlignment.Top;
        IsHitTestVisible = false; Focusable = false;
        RebuildText();
    }
    static Brush CreateInk()
    {
        var brush = new SolidColorBrush(Color.FromRgb(71, 48, 30));
        brush.Freeze(); return brush;
    }
    internal void MarkActivity()
    {
        if (disposed || !IsVisible) return;
        activeUntil = Stopwatch.GetTimestamp() + ActivityTailTicks;
        if (IsSampling) return;
        lastRenderingTime = null; sampleStarted = 0; intervals = 0;
        IsSampling = true;
        CompositionTarget.Rendering += OnRendering;
    }
    void OnRendering(object? sender, EventArgs args)
    {
        long now = Stopwatch.GetTimestamp();
        if (now >= activeUntil || !IsVisible) { Stop(); return; }
        var renderingTime = ((RenderingEventArgs)args).RenderingTime;
        if (renderingTime == lastRenderingTime) return;
        lastRenderingTime = renderingTime;
        if (sampleStarted == 0) { sampleStarted = now; return; }
        intervals++;
        long elapsed = now - sampleStarted;
        if (elapsed < SampleTicks) return;
        LastFramesPerSecond = intervals * (double)Stopwatch.Frequency / elapsed;
        SamplesPublished++;
        SetCaption($"FPS {LastFramesPerSecond.Value.ToString("0.0", CultureInfo.InvariantCulture)}");
        sampleStarted = now; intervals = 0;
    }
    internal void Stop()
    {
        if (IsSampling) CompositionTarget.Rendering -= OnRendering;
        IsSampling = false;
        SetCaption("FPS —");
    }
    void SetCaption(string value)
    {
        if (caption == value) return;
        caption = value; RebuildText(); InvalidateVisual();
    }
    void RebuildText() => text = new FormattedText(caption, CultureInfo.InvariantCulture,
        FlowDirection.LeftToRight, Typeface, 16, Ink, VisualTreeHelper.GetDpi(this).PixelsPerDip);
    protected override void OnDpiChanged(DpiScale oldDpi, DpiScale newDpi)
    {
        base.OnDpiChanged(oldDpi, newDpi); RebuildText(); InvalidateVisual();
    }
    protected override void OnRender(DrawingContext drawingContext)
    {
        base.OnRender(drawingContext);
        drawingContext.DrawText(text, new Point(Math.Max(0, ActualWidth - text.Width), 2));
    }
    public void Dispose() { Stop(); disposed = true; }
}
