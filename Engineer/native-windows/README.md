# Native Epsilon Engineering

Native C# / .NET 10 WPF application. The original browser project in the parent folder is preserved. No WebView, Electron, Node server, installation, game connection, or invented telemetry is used by the native app.

## Launch

Double-click **Start Native Controller.cmd** in this folder, or **dist/Epsilon-Engineering-win-x64/Epsilon Engineering.exe**. The distribution includes the .NET runtime. Copy the entire distribution folder when moving it to another Windows x64 computer. A ZIP of that folder is also supplied.

Starts fullscreen. **F11** returns to an ordinary resizable window and toggles back; **Alt+F4** closes normally. It is not topmost and does not change any other application's settings. `--windowed` starts in a normal window instead.

The reset button is a true circle and follows the selected profile. Click/touch the rail or drag the Brass Grip. Tab moves among the eight sliders, Reset Levels and the rocker. Arrows change one point; Page Up/Down change ten; Home/End select 0/300. Keyboard values snap when focus leaves; pointer/touch values snap on release. Power and coolant positions are independent. Reset affects only the active profile, applying configured defaults exactly, even 99 or 149.

A temporary **FPS** readout is always visible at the top right, with no menus or toggles. While adjusting, switching, resetting or resizing, it reports unique WPF rendering callbacks per second from actual wall-clock intervals, updating at most four times a second. It shows **FPS —** when static and detaches its callback after 400 ms without activity, avoiding a continuous idle render loop. It is a UI scheduling indicator, not a GPU/display-present FPS measurement. Source is isolated in `Epsilon.Native/FpsCounter.cs` for easy removal. See `GPU-REVIEW.md` for the rendering review and limitations.

## Editable configuration

Edit **dist/Epsilon-Engineering-win-x64/config.json** and restart. This is a copy of the existing browser configuration, with version 1, ship name `ESS Endeavour`, the same eight identifiers, power defaults 100 and coolant defaults 0. The app never rewrites it. Its schema and field meanings are unchanged. The ship name appears in the window title.

To use a different file, including the browser's original configuration:

```powershell
& '.\dist\Epsilon-Engineering-win-x64\Epsilon Engineering.exe' --config '..\config.json'
```

All eight entries are required in both profiles; values must be whole numbers 0–300. Invalid JSON, duplicate keys, wrong version, invalid ship name, missing systems or invalid values produce a startup error identifying the configuration path and reason. File reads and JSON parsing run off the UI thread. No defaults are silently substituted.

Coolant positions remain weights. Allocation is `weight / max(300, sum(weights)) * 100`; the ruler maximum is `300 / max(300, sum(weights)) * 100`. Readouts round only for display. No other slider moves when a weight changes.

## Rebuild and test

From this folder, in PowerShell:

```powershell
.\bootstrap.ps1  # only needed without the supplied local SDK or a compatible installed SDK
.\build.ps1
.\verify.ps1
.\benchmark.ps1
```

`build.ps1` restores, builds Release, runs the core tests, and publishes a Windows x64 self-contained folder. `-SkipPublish` builds and runs core tests only. The first build needs Internet access for Microsoft's runtime packs. No third-party NuGet dependencies are used. `bootstrap.ps1` downloads Microsoft's pinned SDK 10.0.403 and verifies its SHA-512; it unpacks under `tools/`, without installing globally. SDK/runtime packs, NuGet cache and CLI configuration stay within this folder. The font and artwork are embedded; the font's SIL license is included beside the executable.

Verification briefly manipulates its own test window and closes it. Its report and a render of the native panel are written under `artifacts/`. The benchmark runs 8 seconds of warmup, 10 seconds idle, 200 switches at absolute 100 ms deadlines over approximately 20 seconds, reset every seventh switch, one second settling, then 10 seconds idle. It closes its window afterward. See **PERFORMANCE.md** for results, limits and remaining physical-device checks.

The optional `benchmark-browser.cjs` is development tooling requiring Node, Playwright and installed Chrome. It launches the unchanged parent browser app in an isolated `tools/benchmark-chrome-profile`, measures it, and closes its own processes. Set `PLAYWRIGHT_MODULE` to an installed Playwright module path if it is not in Node's module search path. This tooling is not needed to run the native app.

## Rendering and future transport

- `Epsilon.Core/Controller.cs`: validated configuration, independent profiles, snapping, exact coolant math, immutable snapshots and piecewise-linear motion geometry. No WPF or network dependency.
- `Epsilon.Native/GripSlider.cs`: native WPF Slider with its RangeValue accessibility peer, explicit mouse/touch capture and keyboard input. A single frozen, downsampled Brass Grip bitmap is shared by all sliders. Cached translation/scale transforms animate; control width, margin and layout are not changed by an animation tick. Fill keyframes include endpoint crossings to clamp exactly during decorative overshoot.
- `PanelWindow.cs`: retained native controls, responsive layout, conditional text updates, shared red/blue brush animations, ordinary fullscreen and Windows reduced-animation preference. There is no procedural texture or idle render loop. Ruler positioning runs only when its text or geometry changes. Rapid retargeting samples the current animated transform. A resized animated slider keeps its visible normalized position and resumes toward its valid target.
- `AdapterPump.cs`: bounded latest-wins queue and a single asynchronous worker. Future I/O stays off the input thread. Implement `IControllerAdapter.ApplyAsync` and replace `LocalAdapter` here to add HTTP later. Errors are surfaced. Snapshots have separate `PowerPercentages`, raw `CoolantWeights`, and calculated `Coolant.Percentages`/`RulerMaximum`; they cannot mutate controller state. No HTTP requests are currently made. A game adapter must separately define ship selection, authoritative state, protocol mapping and capacity/budget handling.
- `Diagnostics.cs`: opt-in self-checks and benchmark instrumentation. Its rendering and layout probes detach afterward. The temporary FPS overlay has its own short-lived activity probe; neither stays subscribed during ordinary idle use.

WPF was selected because the host had no Windows App SDK toolchain and the project-local .NET SDK supports WPF without installing a Windows App SDK/MSIX toolchain. Transform reuse follows Microsoft's [WPF layout/performance guidance](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-layout-and-design). WPF animation is retained-mode but is not claimed to be WinUI's independent compositor animation. Measured results, not framework choice alone, determine the performance claim.
