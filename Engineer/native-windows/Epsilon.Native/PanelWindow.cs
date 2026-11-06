using System.ComponentModel;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using Epsilon.Core;

namespace Epsilon.Native;

public sealed class PanelWindow : Window
{
    public Controller Controller { get; }
    public GripSlider[] Sliders { get; } = new GripSlider[8];
    public bool ReducedMotion => !SystemParameters.ClientAreaAnimation || !SystemParameters.UIEffects;
    public bool IsFullscreen { get; private set; }
    readonly SolidColorBrush accent = new(Color.FromRgb(174, 60, 50));
    readonly SolidColorBrush rail = new(Color.FromRgb(89, 41, 35));
    readonly Brush ink = Brush("#47301E"), muted = Brush("#876749");
    readonly FontFamily panelFont = new(new Uri("pack://application:,,,/"), "./Assets/#Russo One");
    readonly Grid panel = new(), body = new(), footer = new(), ruler = new();
    readonly Canvas tickCanvas = new() { Height = 30 };
    internal FpsCounter FrameCounter { get; } = new();
    readonly TextBlock heading, powerLabel, coolantLabel, resetLabel;
    readonly TextBlock[] readouts = new TextBlock[8], ticks = new TextBlock[4], labels = new TextBlock[8];
    readonly Button reset, rocker;
    readonly Grid resetIcon = new();
    readonly TranslateTransform rockerShift = new();
    readonly AdapterPump adapter;
    bool rendering, animateNext;
    Profile? renderedProfile;
    Rect restoreBounds;
    WindowState restoreState;
    public PanelWindow(Controller controller, bool windowed)
    {
        Controller = controller;
        Title = "Epsilon Engineering · " + controller.ShipName;
        Width = 1280; Height = 800; MinWidth = 640; MinHeight = 480;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        UseLayoutRounding = true; SnapsToDevicePixels = true;
        Background = new LinearGradientBrush(Color.FromRgb(236, 205, 158), Color.FromRgb(207, 174, 124), 70);
        var outer = new Grid(); Content = outer;
        outer.Children.Add(new Border { Margin = new Thickness(14), BorderBrush = Brush("#66785B3B"), BorderThickness = new Thickness(1), IsHitTestVisible = false });
        foreach (var (h, v) in new[] { (HorizontalAlignment.Left, VerticalAlignment.Top), (HorizontalAlignment.Right, VerticalAlignment.Top), (HorizontalAlignment.Left, VerticalAlignment.Bottom), (HorizontalAlignment.Right, VerticalAlignment.Bottom) })
        {
            var screw = new Grid { Width = 13, Height = 13, Margin = new Thickness(9), HorizontalAlignment = h, VerticalAlignment = v, IsHitTestVisible = false };
            screw.Children.Add(new Ellipse { Fill = new LinearGradientBrush(Colors.Gray, Color.FromRgb(40, 43, 41), 130), Stroke = Brush("#3B3A30"), StrokeThickness = 1 });
            screw.Children.Add(new Line { X1 = 3, Y1 = 10, X2 = 10, Y2 = 3, Stroke = Brush("#171C1A"), StrokeThickness = 1 }); outer.Children.Add(screw);
        }
        outer.Children.Add(panel);
        panel.RowDefinitions.Add(new() { Height = GridLength.Auto });
        panel.RowDefinitions.Add(new() { Height = GridLength.Auto });
        panel.RowDefinitions.Add(new() { Height = new GridLength(1, GridUnitType.Star) });
        panel.RowDefinitions.Add(new() { Height = GridLength.Auto });
        heading = Text("ENGINEERING", ink);
        var headerContents = new Grid();
        headerContents.Children.Add(heading); headerContents.Children.Add(FrameCounter);
        var header = new Border { BorderBrush = Brush("#80806443"), BorderThickness = new Thickness(0, 0, 0, 1), Child = headerContents };
        panel.Children.Add(header);
        MakeColumns(ruler); Grid.SetRow(ruler, 1); panel.Children.Add(ruler);
        var scale = new Grid(); Grid.SetColumn(scale, 1); ruler.Children.Add(scale);
        for (int i = 0; i < 4; i++)
        {
            scale.ColumnDefinitions.Add(new() { Width = new GridLength(1, GridUnitType.Star) });
            ticks[i] = Text("", Brush("#735238")); ticks[i].HorizontalAlignment = i == 0 ? HorizontalAlignment.Left : i == 3 ? HorizontalAlignment.Right : HorizontalAlignment.Center;
            // Use fractional positions matching slider rail endpoints, not four equal cells.
        }
        scale.Children.Add(tickCanvas); Grid.SetColumnSpan(tickCanvas, 4);
        foreach (var tick in ticks) tickCanvas.Children.Add(tick);
        tickCanvas.SizeChanged += (_, _) => PlaceTicks();
        Grid.SetRow(body, 2); panel.Children.Add(body);
        for (int i = 0; i < 8; i++)
        {
            int index = i; string id = Systems.Ids[i];
            body.RowDefinitions.Add(new() { Height = new GridLength(1, GridUnitType.Star) });
            var row = new Grid(); MakeColumns(row); Grid.SetRow(row, i); body.Children.Add(row);
            labels[i] = Text(Systems.Labels[i].ToUpperInvariant(), ink);
            var labelBox = new Viewbox { Stretch = Stretch.Uniform, StretchDirection = StretchDirection.DownOnly, HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Center, Child = labels[i], Margin = new Thickness(0, 0, 12, 0) };
            row.Children.Add(labelBox);
            var slider = new GripSlider { Background = rail, Foreground = accent, VerticalAlignment = VerticalAlignment.Stretch };
            AutomationProperties.SetName(slider, Systems.Labels[i] + " Power");
            AutomationProperties.SetAutomationId(slider, id);
            slider.CommitRequested += () => controller.Commit(id);
            slider.ValueChanged += (_, _) => { if (!rendering) controller.Set(id, slider.Value); };
            Grid.SetColumn(slider, 1); row.Children.Add(slider); Sliders[i] = slider;
            readouts[i] = Text("", accent); readouts[i].HorizontalAlignment = HorizontalAlignment.Right;
            var readoutBox = new Viewbox { Stretch = Stretch.Uniform, StretchDirection = StretchDirection.DownOnly, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center, Child = readouts[i] };
            Grid.SetColumn(readoutBox, 2); row.Children.Add(readoutBox);
        }
        var footerFrame=new Border {BorderBrush=Brush("#70806443"),BorderThickness=new Thickness(0,1,0,0),Child=footer};
        Grid.SetRow(footerFrame, 3); panel.Children.Add(footerFrame);
        footer.ColumnDefinitions.Add(new() { Width = GridLength.Auto }); footer.ColumnDefinitions.Add(new() { Width = new GridLength(1, GridUnitType.Star) }); footer.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var resetContents = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        resetIcon.Children.Add(new Ellipse { Fill = accent, Stroke = rail, StrokeThickness = 6 });
        resetIcon.Children.Add(new Ellipse { Margin = new Thickness(7), Stroke = Brush("#88FFFFFF"), StrokeThickness = 2, Fill = new LinearGradientBrush(Color.FromArgb(85,255,255,255),Color.FromArgb(34,0,0,0),135) });
        resetContents.Children.Add(resetIcon); resetLabel = Text("RESET\nLEVELS", accent); resetLabel.Margin = new Thickness(12, 0, 0, 0); resetContents.Children.Add(resetLabel);
        reset = new Button { Style = (Style)FindResource("PlainButton"), Content = resetContents, HorizontalContentAlignment = HorizontalAlignment.Left, MinHeight = 48 };
        AutomationProperties.SetName(reset, "Reset Levels, active profile only"); AutomationProperties.SetAutomationId(reset, "reset"); reset.Click += (_, _) => ResetProfile(); footer.Children.Add(reset);
        var mode = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        powerLabel = Text("POWER", accent); coolantLabel = Text("COOLANT", muted);
        mode.Children.Add(powerLabel);
        var housing = new Border { Width = 82, Height = 58, Padding = new Thickness(6), BorderBrush = Brush("#22291F"), BorderThickness = new Thickness(2), CornerRadius = new CornerRadius(9), Background = Brush("#353B32") };
        var well = new Border { Background = new LinearGradientBrush(Color.FromRgb(38,61,51),Color.FromRgb(69,102,88),90), BorderBrush = Brush("#141E1A"), BorderThickness = new Thickness(1), CornerRadius = new CornerRadius(4), Padding = new Thickness(3) };
        well.Child = new Border { Width = 25, Height = 32, Background = new LinearGradientBrush(Color.FromRgb(167,186,161), Color.FromRgb(228,228,197),0), CornerRadius = new CornerRadius(2), HorizontalAlignment = HorizontalAlignment.Left, RenderTransform = rockerShift };
        housing.Child = well;
        rocker = new Button { Style = (Style)FindResource("PlainButton"), Content = housing, Margin = new Thickness(18,0,18,0), MinHeight = 48 };
        AutomationProperties.SetName(rocker, "Switch to Coolant"); AutomationProperties.SetAutomationId(rocker, "profile-switch");
        rocker.Click += (_, _) => SwitchProfile(); mode.Children.Add(rocker); mode.Children.Add(coolantLabel);
        var modeFit = new Viewbox { Child = mode, Stretch = Stretch.Uniform, StretchDirection = StretchDirection.DownOnly, HorizontalAlignment = HorizontalAlignment.Right };
        Grid.SetColumn(modeFit, 2); footer.Children.Add(modeFit);
        adapter = new AdapterPump(new LocalAdapter(), error => Dispatcher.BeginInvoke(() => MessageBox.Show(this, error.Message, "Adapter error")));
        controller.Changed += OnStateChanged;
        SystemParameters.StaticPropertyChanged += PreferenceChanged;
        SizeChanged += (_, _) => { ResizePanel(); FrameCounter.MarkActivity(); };
        PreviewKeyDown += (_, e) => { if (e.Key == Key.F11) { ToggleFullscreen(); e.Handled = true; } };
        Deactivated += (_, _) => { foreach (var slider in Sliders) slider.EndInteraction(); FrameCounter.Stop(); };
        StateChanged += (_, _) => { if (WindowState == WindowState.Minimized) FrameCounter.Stop(); };
        Closed += (_, _) => { controller.Changed -= OnStateChanged; SystemParameters.StaticPropertyChanged -= PreferenceChanged; FrameCounter.Dispose(); adapter.Dispose(); };
        Loaded += (_, _) => { ResizePanel(); Render(false); adapter.Send(controller.Snapshot()); FrameCounter.MarkActivity(); };
        if (!windowed) EnterFullscreen();
    }
    static Brush Brush(string color) { var brush = (SolidColorBrush)new BrushConverter().ConvertFromString(color)!; brush.Freeze(); return brush; }
    TextBlock Text(string text, Brush foreground) => new() { Text = text, FontFamily = panelFont, Foreground = foreground, VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.NoWrap };
    static void MakeColumns(Grid grid)
    {
        grid.ColumnDefinitions.Add(new() { Width = new GridLength(42, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new() { Width = new GridLength(48, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new() { Width = new GridLength(10, GridUnitType.Star) });
    }
    void PlaceTicks()
    {
        for (int i = 0; i < 4; i++)
        {
            ticks[i].Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            double width = ticks[i].DesiredSize.Width;
            double left = i * tickCanvas.ActualWidth / 3 - (i == 0 ? 0 : i == 3 ? width : width / 2);
            if (Canvas.GetLeft(ticks[i]) != left) Canvas.SetLeft(ticks[i], left);
        }
    }
    void ResizePanel()
    {
        double w = ActualWidth, h = ActualHeight;
        panel.Margin = new Thickness(Math.Clamp(w * .03, 20, 56), Math.Clamp(h * .027, 16, 32), Math.Clamp(w * .03, 20, 56), Math.Clamp(h * .027, 16, 32));
        heading.FontSize = Math.Clamp(Math.Min(w * .058, h * .086), 28, 96);
        heading.Margin = new Thickness(0, 0, 0, Math.Clamp(h * .013, 6, 16));
        ruler.Margin = new Thickness(0, Math.Clamp(h * .017, 6, 18), 0, 4);
        double grip = Math.Clamp(h * .078, 30, 84);
        var scale = (Grid)ruler.Children[0]; scale.Margin = new Thickness(grip/2,0,grip/2,0);
        ((Canvas)scale.Children[0]).Height = Math.Clamp(h * .032, 16, 30);
        foreach (var tick in ticks) tick.FontSize = Math.Clamp(Math.Min(w * .014, h * .027), 10, 25);
        PlaceTicks();
        foreach (var label in labels) label.FontSize = Math.Clamp(Math.Min(w * .033, h * .058), 18, 58);
        foreach (var readout in readouts) readout.FontSize = Math.Clamp(h * .043, 18, 42);
        foreach (var slider in Sliders) slider.SetGeometry(grip);
        footer.Margin = new Thickness(0, Math.Clamp(h * .014, 6, 14), 0, 0);
        footer.Background = Brushes.Transparent;
        resetIcon.Width = resetIcon.Height = Math.Clamp(h * .081, 44, 78);
        resetLabel.FontSize = Math.Clamp(h * .038, 17, 38);
        powerLabel.FontSize = coolantLabel.FontSize = Math.Clamp(Math.Min(w * .026,h * .048), 18, 42);
    }
    void OnStateChanged() { FrameCounter.MarkActivity(); Render(animateNext); adapter.Send(Controller.Snapshot()); }
    public void SwitchProfile()
    {
        foreach (var slider in Sliders) slider.EndInteraction();
        animateNext = true; try { Controller.Switch(); } finally { animateNext = false; }
    }
    public void ResetProfile()
    {
        foreach (var slider in Sliders) slider.EndInteraction();
        animateNext = true; try { Controller.Reset(); } finally { animateNext = false; }
    }
    void Render(bool animate)
    {
        bool modeChanged = renderedProfile != Controller.Active;
        bool coolant = Controller.Active == Profile.Coolant;
        var snapshot = Controller.Snapshot();
        double max = coolant ? snapshot.Coolant.RulerMaximum : 300;
        bool rulerChanged = false;
        for (int i = 0; i < 4; i++) rulerChanged |= SetText(ticks[i], Percent(max * i / 3));
        if (rulerChanged) PlaceTicks();
        if (modeChanged)
        {
            Blend(accent, coolant ? Color.FromRgb(40,123,185) : Color.FromRgb(174,60,50));
            Blend(rail, coolant ? Color.FromRgb(32,63,100) : Color.FromRgb(89,41,35));
            powerLabel.Foreground = coolant ? muted : accent; coolantLabel.Foreground = coolant ? accent : muted;
            double start = rockerShift.X; rockerShift.BeginAnimation(TranslateTransform.XProperty, null); rockerShift.X = coolant ? 28 : 0;
            if (!ReducedMotion) rockerShift.BeginAnimation(TranslateTransform.XProperty, new DoubleAnimation(start, rockerShift.X, TimeSpan.FromMilliseconds(160)) { FillBehavior = FillBehavior.Stop });
            AutomationProperties.SetName(rocker, coolant ? "Switch to Power" : "Switch to Coolant");
        }
        rendering = true;
        try
        {
            for (int i = 0; i < 8; i++)
            {
                string id = Systems.Ids[i]; int target = Controller.Get(id);
                Sliders[i].ReduceMotion = ReducedMotion;
                if (modeChanged || animate || Sliders[i].Value != target) Sliders[i].SetTarget(target, animate);
                SetText(readouts[i], Percent(coolant ? snapshot.Coolant.Percentages[id] : target));
                if (modeChanged) AutomationProperties.SetName(Sliders[i], Systems.Labels[i] + (coolant ? " Coolant weight" : " Power"));
                AutomationProperties.SetHelpText(Sliders[i], $"{readouts[i].Text} {(coolant ? "of shared coolant; position is weight 0 to 300" : "power")}. Arrows adjust one point; focus loss snaps.");
            }
        }
        finally { rendering = false; }
        renderedProfile = Controller.Active;
    }
    static bool SetText(TextBlock node, string value)
    {
        if (node.Text == value) return false;
        node.Text = value; return true;
    }
    static string Percent(double value) => Math.Round(value, MidpointRounding.AwayFromZero).ToString("0") + "%";
    void Blend(SolidColorBrush brush, Color target)
    {
        Color start = brush.Color; brush.BeginAnimation(SolidColorBrush.ColorProperty, null); brush.Color = target;
        if (renderedProfile != null && !ReducedMotion) brush.BeginAnimation(SolidColorBrush.ColorProperty, new ColorAnimation(start, target, TimeSpan.FromMilliseconds(250)) { FillBehavior = FillBehavior.Stop });
    }
    void PreferenceChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (!Dispatcher.CheckAccess()) { Dispatcher.BeginInvoke(() => PreferenceChanged(sender,e)); return; }
        foreach (var slider in Sliders) { slider.ReduceMotion = ReducedMotion; if (ReducedMotion) slider.FinishMotion(); }
        if (ReducedMotion) { accent.BeginAnimation(SolidColorBrush.ColorProperty,null); rail.BeginAnimation(SolidColorBrush.ColorProperty,null); rockerShift.BeginAnimation(TranslateTransform.XProperty,null); }
    }
    void EnterFullscreen()
    {
        restoreBounds = new Rect(Left, Top, Width, Height); restoreState = WindowState;
        WindowState = WindowState.Normal; WindowStyle = WindowStyle.None; ResizeMode = ResizeMode.NoResize; WindowState = WindowState.Maximized; IsFullscreen = true;
    }
    public void ToggleFullscreen()
    {
        if (!IsFullscreen) { EnterFullscreen(); return; }
        WindowState = WindowState.Normal; WindowStyle = WindowStyle.SingleBorderWindow; ResizeMode = ResizeMode.CanResize;
        if (!double.IsNaN(restoreBounds.X)) { Left = restoreBounds.X; Top = restoreBounds.Y; }
        Width = restoreBounds.Width; Height = restoreBounds.Height; WindowState = restoreState; IsFullscreen = false;
    }
}
