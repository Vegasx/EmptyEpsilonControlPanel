# C++ comparison prototype — November 5, 2026

The C++ / Direct2D prototype used about **60 MiB private memory during switching, versus 151 MiB for WPF**, roughly 60% less in these runs. Recorded switching CPU time was lower in both C++ trials, but the process CPU counters were too coarse at this usage level to support a precise speedup ratio. Display smoothness and touch latency were not measured.

## Matched comparison

Evidence: [comparison.json](artifacts/benchmark-20261105-203833/comparison.json), [C++ trial 1](artifacts/benchmark-20261105-203833/cpp-1.json), [C++ trial 2](artifacts/benchmark-20261105-203833/cpp-2.json), [WPF trial 1](artifacts/benchmark-20261105-203833/wpf-1.json), and [WPF trial 2](artifacts/benchmark-20261105-203833/wpf-2.json).

Local AMD Ryzen 7 9800X3D, 16 logical processors, Windows build 19045. Both apps reported a 1000×700-DIP content panel at scale 1.0, with animation enabled. WPF used .NET 10.0.14, hardware rendering tier 2, and its temporary FPS overlay disabled. The C++ build used LLVM-MinGW 20260616 / optimized C++20, Direct2D's default rendering target, and no FPS overlay. Both transports were local/disabled for this rendering comparison. The two trials reversed order: C++ then WPF; WPF then C++. Only one target app ran at a time. Lightweight file inspection and source editing occurred in the task environment; this was not an otherwise isolated laboratory machine.

Each app warmed for six seconds, exercised 30 switches and 60 sinusoidal slider updates, reset and settled for six seconds. Measurement then covered five seconds idle, 120 switches at 10 Hz with 17 resets over 12 seconds, five seconds settling/idle, and 300 sinusoidal slider updates at 30 Hz over ten seconds. The slider-update phase exercises the state/rendering path, not OS pointer delivery. C++ uses deadline-based timers and WPF uses deadline-based asynchronous delays. Neither intentionally renders continuously when settled. The following table uses the mean of the two trials for memory and both individual process-counter readings for CPU.

| Metric | C++ / Direct2D | C# / WPF |
| --- | ---: | ---: |
| Idle private memory, mean | 59.78 MiB | 117.66 MiB |
| End of switching private memory, mean | 59.81 MiB | 150.51 MiB |
| End of slider updates private memory, mean | 59.86 MiB | 153.69 MiB |
| Switching process CPU time, trials 1 / 2 | 93.75 / 0.00 ms | 390.63 / 281.25 ms |
| Slider-update process CPU time, trials 1 / 2 | 31.25 / 0.00 ms | 31.25 / 31.25 ms |
| Initial idle process CPU observed | 0.00 ms in both | 0.00 ms in both |

These are **CPU time**, not elapsed time: each switching phase lasts about 12 seconds in both apps. The counter readings occur in approximately 15.625 ms increments. C++ trial 2 recorded zero CPU while still executing 862 switching draws and 299 slider-update draws; zero is therefore an accounting-resolution limitation, not evidence of literally no CPU work. No numerical CPU speedup ratio is claimed, nor is the slider-update phase treated as a proven CPU improvement. A longer ETW/cycle-based study would better quantify tiny costs.

C++ switching draw-callback intervals were about 15.94 ms median and 16.2 ms p95, with 856 / 862 draws per phase. These are timestamps around the application drawing path, not display-present measurements, and WPF's profile does not provide an equivalent interval metric. They cannot establish superior frame pacing. Both C++ trials recorded zero redraws during the initial idle phase. The five-second phase named `settled` includes the tail of the last switch animation: C++ recorded 18 draws there before stopping. It is not five seconds of entirely static content.

The implementations preserve the artwork, font, slider model and animation geometry; their Win32/Direct2D and WPF drawing pipelines differ and are not pixel-identical. This comparison measures those complete implementations, not an isolated C++ versus C# arithmetic loop. Process private memory excludes GPU allocations and does not measure total system footprint. This CPU/memory run did not collect GPU-engine utilization, energy, thermal, high-refresh presentation or input-to-photon measurements. A subsequent [GPU comparison](GPU-PERFORMANCE.md) measured app-attributed GPU engine activity separately. Two local trials do not guarantee the same results on another machine.

The earlier `benchmark-20261105-203447` directory is explicitly **invalid/excluded**: WPF stayed fullscreen at 2560×1440. The benchmark launcher now passes `--windowed` and rejects unmatched dimensions, display scale or reduced-animation settings before producing a comparison. No production WPF source was changed for this fix.

## Build and functional verification

- Optimized C++ application and tests build with warnings treated as errors; no compiler warnings/errors.
- **9,938 core assertions** cover all integer snapping inputs, randomized coolant budgets/proportions, independent banks, exact reset defaults, full-precision wire values, JSON/configuration validation and interrupted motion.
- **18 in-process UI checks** cover keyboard editing/focus-loss snapping, profile independence, coolant ruler math, active-only reset, endpoint/bounds behavior, capture/release, pointer-handler release, fullscreen restore, rapid switching, interrupted animation, idle animation detachment and layout fit across sizes.
- Rendered [normal](artifacts/cpp-panel.png), [minimum 640×480](artifacts/cpp-panel-minimum.png), [portrait](artifacts/cpp-panel-portrait.png), and [coolant](artifacts/cpp-panel-coolant.png) PNGs were visually inspected. These exercise the shared rendering function through a WIC render target; they are not desktop screenshots or timing evidence.
- [HTTP verification](artifacts/verification.json) passed against a normal receiver and a slow receiver returning 503 errors. Only one request was in flight; intermediate updates were coalesced; all messages had all eight systems; final sequence 302 delivered the snapped reactor value 150. The failed final message retried and succeeded without more user input. The supplied demo receiver also accepted and persisted that final state.
- Missing configuration, invalid endpoint scheme and an explicitly selected missing transport file fail with a diagnostic report. Diagnostics disable outgoing delivery.
- PE imports were checked: the deployed executable uses Windows system DLLs/UCRT and does not import third-party runtime DLLs. The executable is approximately 3.7 MiB; the portable ZIP includes configuration, evidence and license notices.
- The existing WPF Release build and its 5,815 core assertions also passed when preparing the comparison.

Physical touchscreen/pen behavior, simultaneous touches, monitor/DPI transitions, real server/game mapping and screen-reader slider providers remain outside this prototype's verified acceptance. Mouse/touch behavior above was exercised through in-process handlers, not physical input. Keyboard navigation is implemented; the custom panel does not yet expose WPF-equivalent UI Automation sliders. The generic HTTP receiver contract is documented in [README.md](README.md). No live Empty Epsilon integration is claimed.
