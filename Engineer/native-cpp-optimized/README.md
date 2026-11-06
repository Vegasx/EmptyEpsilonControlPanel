# Epsilon Engineering C++ Optimized — separate comparison version

Launch **Start Optimized Cpp Controller.cmd**, or **dist/Epsilon-Engineering-Cpp-Optimized-win-x64/Epsilon Engineering Cpp Optimized.exe**. The title and process name include **Optimized**. The previous native-cpp executable, ZIP and source are preserved; baseline-hashes.json records their original hashes for verification.

This version targets Reset and profile-switch rendering. No performance improvement is claimed until you measure it on the tablet.

- A retained Direct2D surface preserves unchanged pixels. The panel background, heading, system labels, ticks, divider lines and rocker housing are rendered to a reusable background bitmap. During animations, only changed slider/readout regions and the affected footer/ruler/status areas are restored and redrawn. Separate update regions avoid a full-window bounding rectangle. Old grips, overshoot and antialiased edges are restored before new visuals are drawn.
- Changing-color text and the reset-button graphics use bounded, reusable A8 opacity-mask caches. Colors tint the masks during the existing transition. There are no full-screen red/blue crossfade textures. Dynamic text uses grayscale antialiasing for its masks; it can differ slightly from the baseline's text antialiasing. Layout, font, colors and artwork are preserved.
- Reset updates only visuals that actually change. Resetting again while a slider is already heading toward its default preserves that animation's original clock. A settled reset causes no panel redraw unless focus/other visible state changes; the final HTTP state is still submitted.
- A background helper waits for the current monitor's **DXGI vertical blank** and posts at most one outstanding animation notification. It sleeps when no visual change or animation is pending. The old fixed 8 ms animation timer is removed. Direct2D uses immediate presentation with retained contents to avoid waiting for a second refresh after the helper's wait. Windows still performs desktop composition. If a vblank output is unavailable (for example some remote/headless sessions), the helper falls back to the monitor's reported refresh interval. Monitor/display changes refresh the selection.

The same piecewise-linear slider motion, distance-based duration (up to 250 ms), overshoot/settling points, 250 ms color transition and 160 ms rocker transition are used. Pointer dragging remains immediate. The model and transport headers are copied unchanged from the baseline. Power/coolant profiles, snapping, exact reset defaults and the JSON POST contract remain the same.

The additional background and mask caches consume graphics memory; a 1920×1080 four-byte background is approximately 8 MiB before driver overhead. Caches rebuild on resize, DPI change, graphics-device loss or rendering-target replacement. Dirty drawing and presentation are separate: the retained HWND presentation path can still incur driver/compositor copying costs. Reduced animation preferences are respected. Custom screen-reader slider providers and simultaneous multi-finger adjustment remain outside this prototype's scope.

## Controls and connection

Starts fullscreen. F11 toggles a normal resizable window; Alt+F4 closes; `--windowed` starts windowed. Tab cycles eight sliders, Reset and the mode switch. Arrows adjust one point, Page Up/Down ten, Home/End 0/300. Keyboard edits snap on focus loss; pointer edits snap on release. Space/Enter activates the focused button.

The separate distribution has its own **config.json** and **transport.json**, initially copied from the existing C++ distribution. Build preserves these editable files. For a fair comparison, use identical configuration/connection settings in both versions. `--config <path>`, `--transport <path>` and `--endpoint <HTTP/HTTPS URL>` work as before. Network delivery is disabled unless enabled in transport.json or by an endpoint argument. Transport remains one background worker, one request in flight, one newest pending full state, rate-limited moves and retried final delivery. No game-specific adapter is added.

Copy the complete optimized distribution folder to the tablet. It requires Windows 10 1703+ / Windows 11 x64 and system graphics libraries; no .NET, Node or compiler is needed to run it. Keep the original distribution alongside it for comparison.

## Build and checks

From this directory in PowerShell:

```powershell
.\build.ps1
node .\verify.cjs
```

Build reuses the project-local compiler under ../native-cpp/tools (bootstrapping it if needed). Nothing is installed globally. It builds Release with warnings treated as errors, runs the unchanged model tests, and creates the optimized folder and ZIP. `--verify <report.json>` runs functional/rendering checks and closes; `--screenshot <path.png>` renders and closes. `--profile <report.json>` retains the baseline's optional workload mode, under a distinct implementation identifier; normal launches have no permanent FPS or profiling probe. Diagnostics disable HTTP delivery. The user is performing comparative performance profiling independently.

References: [Direct2D caching](https://learn.microsoft.com/en-us/windows/win32/direct2d/improving-direct2d-performance), [A8 masks](https://learn.microsoft.com/en-us/windows/win32/direct2d/compatible-a8-rendertargets), [retained presentation](https://learn.microsoft.com/en-us/windows/win32/api/d2d1/ne-d2d1-d2d1_present_options), [DXGI WaitForVBlank](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgioutput-waitforvblank).
