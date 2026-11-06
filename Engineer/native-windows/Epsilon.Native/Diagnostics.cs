using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Epsilon.Core;

namespace Epsilon.Native;

public static class Diagnostics
{
    static readonly JsonSerializerOptions Json = new() { WriteIndented = true };
    static async Task Save(string path, object report) => await Task.Run(async () => { Directory.CreateDirectory(Path.GetDirectoryName(path)!); await File.WriteAllTextAsync(path, JsonSerializer.Serialize(report, Json)); });
    public static async Task BenchmarkAsync(PanelWindow window, string output)
    {
        await Task.Delay(8000); // Warm up JIT, artwork and font.
        using var process = Process.GetCurrentProcess();
        async Task<object> Measure(bool rapid)
        {
            int switches=0, resets=0;
            var intervals=new List<double>();
            var wallIntervals=new List<double>();
            long? lastWall=null;
            TimeSpan? last=null;
            EventHandler frame=(_,args)=>
            {
                var time=((RenderingEventArgs)args).RenderingTime;
                long wall=Stopwatch.GetTimestamp();
                if(lastWall.HasValue && time!=last) wallIntervals.Add((wall-lastWall.Value)*1000.0/Stopwatch.Frequency);
                if(time!=last) lastWall=wall;
                if(last.HasValue && time>last.Value) intervals.Add((time-last.Value).TotalMilliseconds);
                last=time;
            };
            // Rendering subscription exists ONLY during the active diagnostic phase.
            if(rapid) CompositionTarget.Rendering+=frame;
            long layoutNotifications=0;
            EventHandler layoutChanged=(_,_)=>layoutNotifications++;
            window.LayoutUpdated+=layoutChanged;
            process.Refresh(); double cpuStart=process.TotalProcessorTime.TotalMilliseconds;
            long peakWorking=process.WorkingSet64, peakPrivate=process.PrivateMemorySize64;
            var clock=Stopwatch.StartNew();
            try
            {
                if(rapid)
                    for(int i=1;i<=200;i++)
                    {
                        await Task.Delay(Math.Max(1,(int)(i*100-clock.Elapsed.TotalMilliseconds)));
                        window.SwitchProfile();switches++;
                        if(switches%7==0){window.ResetProfile();resets++;}
                    }
                else await Task.Delay(10000);
            }
            finally { if(rapid) CompositionTarget.Rendering-=frame; window.LayoutUpdated-=layoutChanged; }
            process.Refresh(); double cpu=process.TotalProcessorTime.TotalMilliseconds-cpuStart; double elapsed=clock.Elapsed.TotalMilliseconds;
            peakWorking=Math.Max(peakWorking,process.WorkingSet64);peakPrivate=Math.Max(peakPrivate,process.PrivateMemorySize64);
            var sorted=intervals.Order().ToArray();
            var wallSorted=wallIntervals.Order().ToArray();
            double? Quantile(double q)=>sorted.Length==0?null:sorted[(int)Math.Min(sorted.Length-1,Math.Ceiling(sorted.Length*q)-1)];
            double? WallQuantile(double q)=>wallSorted.Length==0?null:wallSorted[(int)Math.Min(wallSorted.Length-1,Math.Ceiling(wallSorted.Length*q)-1)];
            return new { Phase=rapid?"rapid-switch-10Hz":"idle", ElapsedSeconds=elapsed/1000,Switches=switches,Resets=resets,
                CpuMilliseconds=cpu,CpuPercentOfMachine=cpu/elapsed/Environment.ProcessorCount*100,CpuPercentOfOneLogicalCore=cpu/elapsed*100,
                WorkingSetMiB=process.WorkingSet64/1048576.0,PrivateMiB=process.PrivateMemorySize64/1048576.0,PeakWorkingMiB=peakWorking/1048576.0,PeakPrivateMiB=peakPrivate/1048576.0,
                LayoutNotifications=layoutNotifications,RenderingCallbackIntervals=sorted.Length,CallbackMedianMs=Quantile(.5),CallbackP95Ms=Quantile(.95),CallbackP99Ms=Quantile(.99),CallbackMaximumMs=sorted.Length==0?(double?)null:sorted[^1],WallCallbackMedianMs=WallQuantile(.5),WallCallbackP95Ms=WallQuantile(.95),WallCallbackP99Ms=WallQuantile(.99),WallCallbackMaximumMs=wallSorted.Length==0?(double?)null:wallSorted[^1] };
        }
        var idle=await Measure(false); var rapid=await Measure(true);
        await Task.Delay(1000);
        var recovery=await Measure(false);
        await Save(output,new { Timestamp=DateTimeOffset.Now,Runtime=Environment.Version.ToString(),OS=Environment.OSVersion.ToString(),LogicalProcessors=Environment.ProcessorCount,
            WidthDips=window.ActualWidth,HeightDips=window.ActualHeight,Dpi=VisualTreeHelper.GetDpi(window).PixelsPerInchX,RenderTier=RenderCapability.Tier>>16,window.ReducedMotion,
            Note="Process CPU includes UI/render/GC threads; machine % divides by logical processor count. WPF RenderingTime intervals are nominal frame times. WallCallback intervals measure actual UI callback arrival, not display presentation. Active temporary FPS overlay is included. Both overlay and benchmark frame probes detach at idle. Peak memory is max of phase endpoints, not a continuously sampled high-water mark.", Idle=idle,Rapid=rapid,Recovery=recovery });
        window.Close();
    }
    public static async Task VerifyAsync(PanelWindow window,string output)
    {
        var checks=new List<string>();
        var layouts=new List<object>();
        void Check(bool okay,string name) { if(!okay) throw new Exception("Verification failed: "+name); checks.Add(name); }
        await Task.Delay(500);
        Check(!window.FrameCounter.IsSampling,"temporary FPS counter unsubscribes at startup idle");
        Check(window.IsFullscreen,"fullscreen startup");
        void WindowKey(Key key)=>window.RaiseEvent(new KeyEventArgs(Keyboard.PrimaryDevice,PresentationSource.FromVisual(window)!,0,key){RoutedEvent=Keyboard.PreviewKeyDownEvent});
        WindowKey(Key.F11); await Task.Delay(100);
        Check(!window.IsFullscreen && window.WindowStyle==WindowStyle.SingleBorderWindow,"fullscreen restores normal window");
        window.MaxWidth=4096;window.MaxHeight=4096;
        foreach(var (width,height) in new[]{(1280,800),(1024,768),(800,600),(1920,1080),(768,1024)})
        {
            window.Width=width;window.Height=height; await Task.Delay(100); window.UpdateLayout();
            layouts.Add(new {RequestedWidth=width,RequestedHeight=height,ActualWidth=window.ActualWidth,ActualHeight=window.ActualHeight,MinimumSliderHeight=window.Sliders.Min(s=>s.ActualHeight)});
            Check(window.Sliders.All(s=>s.ActualWidth>0&&s.ActualHeight>0&&s.TranslatePoint(new Point(0,0),window).Y+s.ActualHeight<=window.ActualHeight),$"all 8 controls visible: request {width}x{height}, actual {window.ActualWidth}x{window.ActualHeight}");
        }
        window.Width=1280;window.Height=800; await Task.Delay(150);
        var surface=(FrameworkElement)window.Content;
        var bitmap=new RenderTargetBitmap((int)surface.ActualWidth,(int)surface.ActualHeight,96,96,PixelFormats.Pbgra32);
        var background=new DrawingVisual();using(var drawing=background.RenderOpen())drawing.DrawRectangle(window.Background,null,new Rect(0,0,surface.ActualWidth,surface.ActualHeight));bitmap.Render(background);
        bitmap.Render(surface);
        var encoder=new PngBitmapEncoder();encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using(var imageFile=File.Create(Path.ChangeExtension(output,".png"))) encoder.Save(imageFile);
        window.Controller.Set("reactor",300);window.SwitchProfile();await Task.Delay(40);
        var visible=window.Sliders[0].VisiblePosition; window.SwitchProfile();
        Check(Math.Abs(window.Sliders[0].VisiblePosition-visible)<3,"interrupted profile animation starts from visible position");
        await Task.Delay(300); Check(Math.Abs(window.Sliders[0].VisiblePosition-300)<1e-8,"animation lands exactly");
        for(int i=0;i<120;i++) { window.SwitchProfile(); if(i%3==0)window.ResetProfile();await Task.Delay(10); }
        await Task.Delay(300);Check(window.Sliders.All(s=>Math.Abs(s.VisiblePosition-s.Value)<1e-8),"120 rapid toggles with reset settle exactly");
        Check(window.FrameCounter.LastFramesPerSecond is > 0 && window.FrameCounter.SamplesPublished > 0,"FPS counter reports a finite positive rate during repeated toggles");
        await Task.Delay(200);Check(!window.FrameCounter.IsSampling,"FPS counter detaches after interaction settles");
        var slider=window.Sliders[0];slider.Focus();window.Controller.Set("reactor",100);
        // In-process routed touch events exercise the real handlers/capture, without OS input injection.
        double gripWidth=Math.Clamp(window.ActualHeight*.078,30,84);
        double travel=slider.ActualWidth-gripWidth;
        using(var touch=new DiagnosticTouch(slider,new Point(gripWidth/2+travel*.7,slider.ActualHeight/2)))
        {
            touch.Send(UIElement.PreviewTouchDownEvent,TouchAction.Down);
            Check(window.Controller.Get("reactor")==210,"touch track press sets underlying position immediately");
            touch.Position=new Point(gripWidth/2+travel*199/300,slider.ActualHeight/2);
            touch.Send(UIElement.PreviewTouchMoveEvent,TouchAction.Move);
            Check(window.Controller.Get("reactor")==199,"touch drag remains unsnapped");
            touch.Send(UIElement.PreviewTouchUpEvent,TouchAction.Up);
            Check(window.Controller.Get("reactor")==200,"touch release applies strong snapping");
        }
        window.Controller.Set("reactor",100);
        void SendKey(Key key) { var source=PresentationSource.FromVisual(slider)!;slider.RaiseEvent(new KeyEventArgs(Keyboard.PrimaryDevice,source,0,key) {RoutedEvent=Keyboard.PreviewKeyDownEvent}); }
        SendKey(Key.Right); SendKey(Key.Right);Check(slider.Value==102,"keyboard escapes major snap stop by one-point steps");
        slider.MoveFocus(new TraversalRequest(FocusNavigationDirection.Next));Check(window.Controller.Get("reactor")==100,"keyboard snaps only on focus loss");
        slider.ReduceMotion=true;slider.SetTarget(300,true);Check(Math.Abs(slider.VisiblePosition-300)<1e-8,"reduced-animation slider immediately reaches target");slider.ReduceMotion=window.ReducedMotion;
        var before=window.Controller.Snapshot();window.ResetProfile();var after=window.Controller.Snapshot();
        Check(Systems.Ids.All(id=>window.Controller.Active==Profile.Power?before.CoolantWeights[id]==after.CoolantWeights[id]:before.PowerPercentages[id]==after.PowerPercentages[id]),"reset preserves inactive bank");
        WindowKey(Key.F11);await Task.Delay(100);Check(window.IsFullscreen,"F11 fullscreen re-entry");
        await Save(output,new {Checks=checks,Layouts=layouts,Dpi=VisualTreeHelper.GetDpi(window).PixelsPerInchX,NotVerified=new[]{"physical touch hardware", "real Windows display scale changes", "high-refresh presentation timestamps", "mouse capture through OS input (separate manual UI check)","OS Alt+F4 shortcut"}});
        window.Close();
    }
    sealed class DiagnosticTouch : TouchDevice, IDisposable
    {
        readonly FrameworkElement target;
        TouchAction action;
        public Point Position {get;set;}
        public DiagnosticTouch(FrameworkElement target,Point position):base(0xE551)
        {this.target=target;Position=position;SetActiveSource(PresentationSource.FromVisual(target));Activate();}
        public void Send(RoutedEvent routedEvent,TouchAction touchAction)
        {action=touchAction;target.RaiseEvent(new TouchEventArgs(this,Environment.TickCount){RoutedEvent=routedEvent});}
        public override TouchPoint GetTouchPoint(IInputElement? relativeTo)
        {var point=relativeTo is UIElement element?target.TranslatePoint(Position,element):Position;return new TouchPoint(this,point,new Rect(point,new Size(1,1)),action);}
        public override TouchPointCollection GetIntermediateTouchPoints(IInputElement? relativeTo)=>new() {GetTouchPoint(relativeTo)};
        public void Dispose(){Capture(null);Deactivate();}
    }
}
