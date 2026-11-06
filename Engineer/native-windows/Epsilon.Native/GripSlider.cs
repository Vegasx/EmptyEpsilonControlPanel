using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Imaging;
using Epsilon.Core;

namespace Epsilon.Native;

public sealed class GripSlider : Slider
{
    static readonly BitmapSource Artwork = LoadArtwork();
    FrameworkElement? rail;
    Image? grip;
    TranslateTransform? translation;
    ScaleTransform? fill;
    bool programmatic, keyboardEditing, dragging;
    int? touchId;
    double dragOffset;
    double gripSize = 64;
    double previousTravel;
    public bool ReduceMotion { get; set; }
    public event Action? CommitRequested;
    public double VisiblePosition => previousTravel <= 0 ? Value : (translation?.X ?? 0) / previousTravel * 300;
    double Travel => Math.Max(0, ActualWidth - gripSize);
    static BitmapSource LoadArtwork()
    {
        var bitmap = new BitmapImage(); bitmap.BeginInit();
        bitmap.UriSource = new Uri("pack://application:,,,/Assets/BrassGrip.png");
        bitmap.DecodePixelWidth = 192; bitmap.CacheOption = BitmapCacheOption.OnLoad;
        bitmap.EndInit(); bitmap.Freeze(); return bitmap;
    }
    public GripSlider()
    {
        Minimum = 0; Maximum = 300; SmallChange = 1; LargeChange = 10;
        IsMoveToPointEnabled = true;
        ValueChanged += (_, _) => { if (!programmatic) PositionImmediately(Value); };
        SizeChanged += (_, _) => ResizeMotion();
        LostKeyboardFocus += (_, _) => { if (keyboardEditing) CommitRequested?.Invoke(); keyboardEditing = false; };
        LostMouseCapture += (_, _) => { if (dragging && touchId == null) FinishDrag(); };
        LostTouchCapture += (_, e) => { if (touchId == e.TouchDevice.Id) { touchId = null; FinishDrag(); } };
    }
    public override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        rail = (FrameworkElement)GetTemplateChild("PART_Rail");
        grip = (Image)GetTemplateChild("PART_Grip"); grip.Source = Artwork;
        // Template Freezables may be frozen/shared; each slider owns mutable transforms.
        translation = new TranslateTransform(); grip.RenderTransform = translation;
        fill = new ScaleTransform(); ((FrameworkElement)GetTemplateChild("PART_Fill")).RenderTransform = fill;
        SetGeometry(gripSize); PositionImmediately(Value);
    }
    public void SetGeometry(double size)
    {
        if (Math.Abs(size-gripSize)<.001 && previousTravel>0) return;
        double visible=VisiblePosition;
        bool moving=translation?.HasAnimatedProperties == true;
        gripSize = size;
        if (grip != null) { grip.Width = size; grip.Height = size; }
        if (rail != null) rail.Margin = new Thickness(size / 2, 0, size / 2, 0);
        previousTravel=Travel;
        PositionImmediately(moving?visible:Value);
        if(moving) SetTarget((int)Value,true);
    }
    void ResizeMotion()
    {
        double visible=VisiblePosition;bool moving=translation?.HasAnimatedProperties==true;
        previousTravel=Travel;PositionImmediately(moving?visible:Value);
        if(moving) SetTarget((int)Value,true);
    }
    public void FinishMotion() => PositionImmediately(Value);
    public void SetTarget(int target, bool animate)
    {
        double start = VisiblePosition;
        programmatic = true;
        try { Value = target; } finally { programmatic = false; }
        if (translation == null || fill == null) return;
        previousTravel=Travel;
        if (!animate || ReduceMotion || Math.Abs(target - start) < .01 || Travel <= 0)
        { PositionImmediately(target); return; }
        var points = Motion.Points(start, target);
        Animate(translation, TranslateTransform.XProperty, points, Travel / 300);
        Animate(fill, ScaleTransform.ScaleXProperty, Motion.ClampedPoints(points), 1.0 / 300);
    }
    static void Animate(Animatable destination, DependencyProperty property, IReadOnlyList<MotionPoint> points, double factor)
    {
        var animation = new DoubleAnimationUsingKeyFrames { FillBehavior = FillBehavior.Stop };
        foreach (var point in points)
            animation.KeyFrames.Add(new LinearDoubleKeyFrame(point.Value * factor, KeyTime.FromTimeSpan(TimeSpan.FromMilliseconds(point.Milliseconds))));
        // Base value is the exact endpoint; expired clocks do not hold the visual.
        destination.SetValue(property, points[^1].Value * factor);
        destination.BeginAnimation(property, animation, HandoffBehavior.SnapshotAndReplace);
    }
    void PositionImmediately(double value)
    {
        if (translation == null || fill == null) return;
        translation.BeginAnimation(TranslateTransform.XProperty, null);
        fill.BeginAnimation(ScaleTransform.ScaleXProperty, null);
        translation.X = value / 300 * Travel; fill.ScaleX = Math.Clamp(value / 300, 0, 1);
    }
    void BeginDrag(double x)
    {
        Focus(); keyboardEditing = false;
        double visible = VisiblePosition;
        double center = gripSize / 2 + visible / 300 * Travel;
        dragOffset = Math.Abs(x - center) <= gripSize / 2 ? x - center : 0;
        PositionImmediately(visible); dragging = true;
        UpdateDrag(x);
    }
    void UpdateDrag(double x)
    { if (Travel > 0) Value = Rules.Position((x - gripSize / 2 - dragOffset) / Travel * 300); }
    void FinishDrag()
    { if (!dragging) return; dragging = false; CommitRequested?.Invoke(); }
    public void EndInteraction()
    {
        if (dragging) FinishDrag();
        touchId = null; ReleaseMouseCapture(); ReleaseAllTouchCaptures();
        if (keyboardEditing) CommitRequested?.Invoke(); keyboardEditing = false;
    }
    protected override void OnPreviewMouseLeftButtonDown(MouseButtonEventArgs e)
    { if (touchId != null) { e.Handled = true; return; } BeginDrag(e.GetPosition(this).X); CaptureMouse(); e.Handled = true; }
    protected override void OnPreviewMouseMove(MouseEventArgs e)
    { if (dragging && touchId == null && e.LeftButton == MouseButtonState.Pressed) { UpdateDrag(e.GetPosition(this).X); e.Handled = true; } }
    protected override void OnPreviewMouseLeftButtonUp(MouseButtonEventArgs e)
    { if (dragging && touchId == null) { UpdateDrag(e.GetPosition(this).X); FinishDrag(); ReleaseMouseCapture(); } e.Handled = true; }
    protected override void OnPreviewTouchDown(TouchEventArgs e)
    {
        if (touchId == null) { touchId = e.TouchDevice.Id; BeginDrag(e.GetTouchPoint(this).Position.X); CaptureTouch(e.TouchDevice); }
        e.Handled = true;
    }
    protected override void OnPreviewTouchMove(TouchEventArgs e)
    { if (touchId == e.TouchDevice.Id) UpdateDrag(e.GetTouchPoint(this).Position.X); e.Handled = true; }
    protected override void OnPreviewTouchUp(TouchEventArgs e)
    {
        if (touchId == e.TouchDevice.Id) { UpdateDrag(e.GetTouchPoint(this).Position.X); FinishDrag(); touchId = null; ReleaseTouchCapture(e.TouchDevice); }
        e.Handled = true;
    }
    protected override void OnPreviewKeyDown(KeyEventArgs e)
    {
        double? value = e.Key switch { Key.Left or Key.Down => Value - 1, Key.Right or Key.Up => Value + 1,
            Key.PageDown => Value - 10, Key.PageUp => Value + 10, Key.Home => 0, Key.End => 300, _ => null };
        if (value.HasValue) { keyboardEditing = true; PositionImmediately(Value); Value = Rules.Position(value.Value); e.Handled = true; }
        else base.OnPreviewKeyDown(e);
    }
    protected override void OnMouseWheel(MouseWheelEventArgs e) { e.Handled = true; }
}
