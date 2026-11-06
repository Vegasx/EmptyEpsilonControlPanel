# Rendering review and temporary FPS overlay

## Findings

No obvious heavyweight GPU operation is present in the native controller. There are no blur/shadow effects, opacity masks, procedural texture, WebView surfaces, per-frame image decoding or full-panel bitmap caches. Handles already share a frozen 192-pixel decoded bitmap; motion uses existing translate/scale transforms and the rails remain bounded. Microsoft's [WPF imaging guidance](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-2d-graphics-and-imaging) supports decoding to an appropriate size and choosing a speed-oriented image scaling mode, both already implemented here.

| Priority | Avenue | Evidence and practical next step |
| --- | --- | --- |
| First GPU experiment | Profile the profile-color blend | `PanelWindow.Blend` animates two shared brushes used by all rails, readouts and reset graphics for 250 ms. This is the broadest continuously changing part of the panel during toggling. A trace should establish whether brush updates, text drawing, GPU fill, or composition dominate. If text/color painting proves expensive, A/B test small pre-rendered red/blue readout layers with opacity blending while retaining the required linear color transition. This adds textures and overdraw, so it is a candidate, not an assumed improvement. |
| Next, if GPU fill is the bottleneck | Keep any added caching local | A small, complex, repeatedly transformed visual can benefit from caching; the Brass Grip is already a bitmap and gains little from another bitmap cache. Avoid caching the whole panel or frequently recolored slider rows: changing descendants can invalidate caches and high-DPI surfaces increase memory/fill cost. Microsoft's [BitmapCache documentation](https://learn.microsoft.com/en-us/dotnet/api/system.windows.media.bitmapcache) describes caching complex subtrees in video memory; it is not a blanket recommendation for this simple panel. |
| If UI/animation scheduling is the bottleneck | Consider independent compositor animation in a future WinUI renderer | WPF's retained rendering does not make every animation independent of its UI thread. More GPU capacity will not remove UI-thread stalls. A renderer migration is a larger experiment, warranted only after a CPU/GPU trace shows that constraint. No framework migration or frame-rate cap was introduced in this change. |

The first useful profiling pass is an ETW CPU/GPU trace during rapid toggling and dragging. [GPUView](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/using-gpuview) can distinguish CPU scheduling/command submission from GPU processing. Hardware rendering tier alone does not establish that the app is GPU-bound. GPU utilization, GPU queue duration, display-present timestamps and GPU memory were not measured in this review, so no GPU throughput or energy improvement is claimed.

## Implemented cleanup

The previous ruler subscribed to `LayoutUpdated`, built a joined string of all four tick labels on every notification, and compared it before positioning ticks. That work also ran during animation frames unrelated to ruler changes. Ruler positioning now runs only when its text, font size or available width changes, and avoids redundant position assignments.

The always-attached diagnostic layout-notification counter was moved into the opt-in benchmark and detached after each phase. These changes remove unnecessary UI-thread work and allocations; they are **CPU-side cleanup**, not a measured GPU optimization. No changes were made to the artwork, panel appearance, slider focus-box removal, animations, configuration or allocation behavior.

Sources: `Epsilon.Native/PanelWindow.cs` (`PlaceTicks`, `Render`, `Blend`), `Epsilon.Native/GripSlider.cs` (`LoadArtwork`, `Animate`), `Epsilon.Native/App.xaml` (slider visuals) and `Epsilon.Native/Diagnostics.cs` (benchmark-only probes).

## Temporary FPS readout

`Epsilon.Native/FpsCounter.cs` adds one small, non-interactive readout at the top right of the existing header. There are no toggles, menus or settings. It counts unique WPF `CompositionTarget.Rendering` callbacks against Stopwatch wall time, excluding duplicate callbacks for the same rendering timestamp. Text updates at most every 250 ms. Its fixed-size drawing invalidates only its own visual and does not remeasure the header whenever the number changes.

The display reads `FPS 60.0`, for example, during adjustment, profile switching, reset or resize. It shows `FPS —` when static. Sampling detaches 400 ms after the most recent activity, and also on deactivation, minimization or window close. There is no permanent sampling timer or idle frame subscription. The counter remains visible while static, but does not force the controller to keep rendering. This keeps the original idle-efficiency requirement while providing a useful motion diagnostic.

This is **WPF callback FPS**, not GPU/display-present FPS or monitor refresh rate. A UI callback can occur without a newly presented frame, and a GPU/compositor stall is not necessarily reflected by the callback count. Microsoft's [Rendering event documentation](https://learn.microsoft.com/en-us/dotnet/api/system.windows.media.compositiontarget.rendering) places the event in WPF's scene update/rendering sequence. The overlay itself adds a small amount of work while sampling; it is temporary instrumentation, not a zero-overhead benchmark.

To remove it later: remove `FpsCounter.cs`, its field/header child and activity/cleanup calls in `PanelWindow.cs`, and the three overlay-specific checks in `Diagnostics.cs`. There are no configuration switches or persisted preferences to clean up.

## Verification

Release compilation and the original **5,815 core assertions** passed. The native window passed **21 checks**, including positive FPS samples during repeated toggles, startup-idle detachment and detachment after interaction, plus the existing profile, snapping, touch-handler, keyboard, reset, resize and fullscreen checks. The rendered preview verifies the counter's top-right placement and preserved panel appearance.

Evidence: [build log](artifacts/fps-build.log), [UI checks](artifacts/fps-verification.json), [preview](artifacts/fps-verification.png), and [follow-up performance run](artifacts/fps-performance.json). Historical results in `PERFORMANCE.md` remain labeled as measurements of the earlier build.

The follow-up run used 8 seconds warmup, 10 seconds startup-idle observation, 200 switches and 28 resets in 20 seconds, a one-second settling interval and 10 seconds settled idle. Native process CPU measured 3.59% during the first idle observation, 9.04% during toggling, and 0.00% observed during settled idle, on a total-machine basis across 8 logical processors. Startup-idle recorded 60 layout notifications, while settled idle had zero. The first idle phase therefore still included transient work; its source was not isolated. It is not reported as a stable near-zero startup measurement.

At the end of toggling, private memory was 90.7 MiB and working set 166.3 MiB. Rendering callback wall intervals were 16.87 ms median, 30.51 ms p95, 46.94 ms p99 and 67.24 ms maximum. These results include the temporary overlay and the active benchmark probe. They do not establish an improvement over the old build or isolate the overlay's overhead: the display orientation, system load and instrumentation differ, and this was one local run. Settled-idle sampling detachment was verified independently by the UI checks. Observed zero CPU is limited by the process counter's resolution.

The current display is landscape, unlike the earlier portrait baseline. Requested 1280×800 and 1024×768 windows now fit at their requested sizes; larger 1920×1080 and taller 768×1024 requests remain bounded by the current monitor. The report records actual dimensions and 192 DPI. Physical touch/mouse, changing display scales, high-refresh presentation and OS Alt+F4 delivery retain the previously documented limits.
