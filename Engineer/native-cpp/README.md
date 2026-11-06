# Epsilon Engineering — C++ comparison prototype

Windows x64 C++20 / Win32 / Direct2D / DirectWrite controller. The Russo One font and Brass Grip artwork are embedded. The C# and browser applications are preserved. The deployed app uses Windows system libraries; it needs no .NET, Node, browser, compiler, installation, or third-party runtime DLLs.

Double-click **Start Cpp Controller.cmd**, or **dist/Epsilon-Engineering-Cpp-win-x64/Epsilon Engineering Cpp.exe**. Copy the whole distribution folder to another Windows 10 (1703+) or Windows 11 x64 computer. Starts fullscreen; F11 toggles a normal resizable window and Alt+F4 closes. `--windowed` starts windowed. Eight sliders retain independent power and coolant banks. Drag/click/touch/pen to move; release snaps. Tab cycles eight sliders, reset, and profile switch. Arrows change one point, Page Up/Down ten, Home/End 0/300. Keyboard edits snap on focus loss. Space/Enter activates the focused button. Reset uses the active bank's exact configured defaults. The system reduced-animation preference is respected; `--reduced-motion` also disables animation.

The renderer redraws on changes and during finite animations. Its animation timer detaches when settled. There is no permanent idle render loop or FPS overlay. Rendering resources, decoded artwork and text layouts are reused. DPI changes, resize, interrupted animations, mouse capture and one active touch/pen interaction are handled. Simultaneous multi-finger adjustments are outside this prototype's scope. It is a custom-drawn panel with keyboard navigation; it does not yet expose WPF-equivalent screen-reader slider providers.

## Server connection

The supplied **transport.json** is disabled. To connect, edit **dist/Epsilon-Engineering-Cpp-win-x64/transport.json**, set `enabled` to `true` and `endpoint` to your receiver's full HTTP/HTTPS URL, then restart. `sendHz` is 1–120 (default 30). Alternatively launch with `--endpoint http://127.0.0.1:4181/controller`. Configuration is read relative to the executable, not the current directory. `--config <path>` and `--transport <path>` select other files. The original config.json schema is preserved; connection settings are separate. The top-right status reports LOCAL, CONNECTING, HTTP OK, or the most recent retry/error.

This is a **generic JSON POST receiver contract**, not an implemented Empty Epsilon game adapter. The real receiver still needs to select a ship, map quantities and reconcile authoritative server state. No unknown game endpoint or coolant-capacity conversion is assumed. Power values are percentages 0–300, not multipliers; divide by 100 if the receiver expects 0–3. Coolant weights remain raw 0–300. Calculated percentages use `weight / max(300, sum(weights)) * 100`; rulerMaximum is `300 / max(300, sum(weights)) * 100`.

For a working local demo, run `node .\demo-receiver.cjs` in this source directory, then launch `.\"Start Cpp Controller.cmd" --endpoint http://127.0.0.1:4181/controller`. The receiver prints updates and saves the newest complete state to artifacts/demo-latest-state.json. It binds only to this computer, requires no packages, and stops with Ctrl+C. Node is needed only for this development receiver, not for the controller or another server implementation.

Each POST has Content-Type `application/json; charset=utf-8`:

```json
{
  "version": 1,
  "shipName": "ESS Endeavour",
  "sequence": 42,
  "final": true,
  "powerPercentages": { "reactor": 100 },
  "coolantWeights": { "reactor": 150 },
  "coolant": {
    "totalWeight": 150,
    "rulerMaximum": 100.0,
    "percentages": { "reactor": 50.0 }
  }
}
```

The example abbreviates the maps: actual messages always include all eight system identifiers. A dedicated background thread owns the WinHTTP session, with one request in flight and one newest pending full snapshot. Intermediate moves are coalesced and rate limited. Release, keyboard focus loss, reset and startup submit a final snapshot that bypasses the move-rate delay. In-flight I/O still finishes first. Switching profiles changes only the view. Requests are serialized; sequence increases per process run and resets on restart. A receiver should apply each full state idempotently: retries can duplicate delivery. Multiple controllers/session conflict resolution is not provided. An HTTP 2xx acknowledges transport delivery, not proof that a game applied the values.

Failures retain the latest state and retry with 250 ms–4 s exponential backoff. New values replace a failed pending snapshot. Connection/send/receive timeouts are 1 s each. The app uses direct connections (no proxy), standard HTTPS certificate validation and no implicit Windows credential negotiation or redirects. No authentication headers are supplied by this prototype. Return 204 or a small body to reuse the connection. Closing cancels pending work and waits for the bounded in-flight operation; it does not guarantee a final server delivery during shutdown. A failed final update continues retrying while the app stays open.

## Build and verification

From this directory in PowerShell:

```powershell
.\bootstrap.ps1
.\build.ps1
node .\verify.cjs
.\benchmark.ps1
.\profile-gpu.ps1
```

The bootstrap downloads LLVM-MinGW 20260616 into tools/ and verifies its upstream SHA-256. Nothing is installed globally. Build uses optimized C++20, static C++ runtime, Windows API libraries, and warnings as errors. Artifacts are under dist/, build/ and artifacts/. The compiler and SDK headers/import libraries are development-only. The LLVM, MinGW runtime and font license notices are included in the distribution; compiler notices also remain in tools/.

Verification covers model/configuration invariants, animation retargeting, in-process UI handlers, a rendered PNG, and real loopback HTTP delivery including slow/failing receivers and a final-state retry. UI handler tests are not physical OS input tests. `--verify <report.json>` and `--screenshot <image.png>` run briefly and close. They disable network delivery, as does `--profile <report.json>`. Diagnostics never contact a configured receiver.

The benchmark uses the same warmed 1000×700-DIP workload as the WPF profiling mode: 30 switches, 60 slider updates, six seconds settling, then idle, 120 switches/17 resets, settled idle, and 300 updates at 30 Hz. It measures process CPU/memory; C++ draw-call intervals are not display-present FPS. Physical touchscreen, pen, monitor changes, high refresh, screen-reader support and actual game integration need separate acceptance checks. See PERFORMANCE.md for measured results and limitations.

`profile-gpu.ps1` runs two matched pairs in reverse order and samples Windows GPU Engine active-time counters every 250 ms in a separate native helper. Only the target PID's counters are retained. `summarize-gpu.cjs` validates timestamps, counter types, coverage, dimensions, animation settings and workload counts, then writes comparison-gpu.json. The C++ profile report includes UTC phase boundaries and process IDs for counter attribution. The production renderer and server protocol are unchanged. See GPU-PERFORMANCE.md for GPU results and the distinction between summed engine busy time and display latency.

Implementation references: [Direct2D QuickStart](https://learn.microsoft.com/en-us/windows/win32/direct2d/getting-started-with-direct2d), [WinHTTP POST](https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpsendrequest), [pointer input](https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerdown).
