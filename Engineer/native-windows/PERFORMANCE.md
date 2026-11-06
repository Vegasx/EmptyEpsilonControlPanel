# Native verification and performance — 2026-11-05

Historical baseline: the measurements below precede the temporary top-right FPS overlay and ruler polling cleanup. See `GPU-REVIEW.md` and `artifacts/fps-performance.json` for the follow-up build; these older results are not silently relabeled as current measurements.

The native app is implemented and runs as a self-contained Windows x64 WPF application. The browser source, configuration, font, license, artwork and preview were preserved; SHA-256 checks confirm all 13 recorded original files are unchanged. Nothing was installed globally and normal Chrome profiles/settings were not changed.

## Measured comparison

Local Intel Core i7-1065G7, 8 logical processors; Windows build 26300; 200% display scaling. Native .NET 10.0.14, WPF hardware rendering tier 2. Installed headful Chrome 157 ran the unchanged browser source in an isolated test profile. Native content was 912×1368 DIPs; Chrome content was 912×1312 CSS pixels at devicePixelRatio 2 on the same 912×1368 logical screen. Chrome retained 56 pixels of browser chrome despite requesting fullscreen; this is a limitation of the comparison, not a perfectly matched rendering surface. No display, Chrome or power settings were changed.

Each final run used 8 seconds warmup, about 10 seconds idle, 200 profile switches over 20 seconds (100 ms target cadence), reset every seventh switch (28 resets), one second settling, then about 10 seconds idle. Both started with the same 100 power / 0 coolant defaults. Native uses absolute deadlines to avoid accumulating DispatcherTimer quantization. Tests were run sequentially. These are single local runs, not controlled repeated trials or a guarantee for another computer.

| Metric | Native WPF | Existing Chrome controller |
| --- | ---: | ---: |
| Idle CPU, total-machine basis | 0.04% | 0.08% |
| Rapid-switch CPU, total-machine basis | 7.45% | 11.45% |
| Settled CPU, total-machine basis | 0.00% observed | 0.32% |
| Idle private memory | 62 MiB | 257 MiB |
| End of switching, private memory | 89 MiB | 336 MiB |
| End of switching, working set | 164 MiB | 606 MiB* |
| Callback wall interval median | 16.67 ms | 16.70 ms |
| Callback wall interval p95 | 18.27 ms | 17.30 ms |
| Callback wall interval p99 | 21.07 ms | 18.20 ms |
| Longest observed callback interval | 42.68 ms | 19.40 ms |

Native used less CPU and private memory in this test. Chrome had steadier callback timing. The result does **not** establish superior native frame pacing or reproduce the user's reported roughly 20% Chrome process usage. The local baseline measured about 11.45% across Chrome's own browser/renderer/GPU/utility processes; Node's static server and the test harness are excluded. Native CPU covers its full process, including UI, rendering and GC. Machine percentages divide cumulative CPU time by wall time and 8 logical processors; native switching was 59.61% of one logical core and Chrome 91.56%.

Native wall intervals use Stopwatch timestamps at WPF CompositionTarget.Rendering callbacks; browser intervals use performance.now at requestAnimationFrame callbacks. WPF's nominal RenderingTime intervals are recorded separately in JSON and should not be mistaken for actual arrival times. Neither probe measures display presentation, GPU completion, dropped presents, touch latency or input-to-photon delay. The probes add overhead during active measurement. Native unsubscribes its frame probe afterward. There is no idle diagnostic timer, production rendering loop or ongoing frame subscription. Both idle phases had zero WPF LayoutUpdated notifications; active LayoutUpdated notifications are **not** interpreted as counts of actual measure/arrange passes. Observed zero CPU is below the counter's resolution for that interval, not proof of literally zero work.

*Chrome working sets are summed across processes and include shared pages more than once. Private memory is the more useful process-memory comparison here. Reported phase peak memory is only the maximum of endpoints, not a continuously sampled high-water mark. No GPU-memory, energy or thermal measurements were taken. Tier 2 indicates available hardware rendering, not an FPS guarantee. High-refresh displays were not tested.

Raw evidence: [native-performance.json](artifacts/native-performance.json), [browser-performance.json](artifacts/browser-performance.json), [build-and-tests.log](artifacts/build-and-tests.log), [interaction-verification.json](artifacts/interaction-verification.json), [native panel render](artifacts/interaction-verification.png), and [preservation check](artifacts/browser-preservation-check.json). `benchmark.ps1` reproduces native measurements; `benchmark-browser.cjs` supplies the optional browser harness. Screenshots/renders are visual QA, not timing evidence.

## Verified behavior

Release build: no compiler warnings or errors. Six core-test groups passed, with **5,815 assertions**, including every integer snapping input 0–300, inclusive strong-snap boundaries/priority/idempotence, coolant examples and 500 randomized budgets, unchanged weights, invalid configuration forms/entries, immutable adapter snapshots, active-only reset for both profiles, unsnapped custom defaults, exact overshoot geometry/timing and clamped fills.

The runnable self-contained app passed **18 in-process UI checks**: fullscreen startup; F11 restoring/re-entering a normal window; all eight controls visible during resize; interruption continuity; exact animation completion; 120 rapid interrupted toggles with resets; native routed touch press/move/release with capture, unsnapped dragging and release snapping; precise keyboard escape from a snap point and focus-loss commit; reduced-animation immediate slider completion; and inactive-bank preservation on reset. These touch checks use a WPF TouchDevice test double through real routed handlers, not physical touchscreen input.

Resizing was exercised at the current 192 DPI / 200% scaling. Requested widths 1280, 1024 and 1920 were constrained to an actual 927 DIPs by this portrait monitor's window tracking limits. Actual verified outer sizes were 927×800, 927×768, 800×600, 927×1080 and 768×1024, with minimum slider heights from 46 to 93 DIPs. The JSON records requested and actual sizes separately. Full 1920-wide physical desktop testing remains outstanding.

## Remaining device checks

Physical touchscreen/pen behavior and multi-touch; real mouse capture and dragging through OS input; moving between monitors and changing Windows display scale; full-width landscape desktops; >60 Hz presentation; and OS Alt+F4 delivery remain unverified. The optional computer-use tool timed out waiting for app access, so no successful OS mouse test is claimed. F11 was verified through the app's routed key handler. Alt+F4 uses ordinary Windows window closing, with no interception or kiosk lock. Windows' current animation preference was read; slider animation suppression was exercised in-process, but the user's system preference was not changed for testing.

Suggested device acceptance check: launch, drag each slider using mouse and touch (including off-rail capture and a second finger), toggle/reset during motion, Tab and arrow away from 100, verify only focus loss snaps, reset each bank, resize to landscape and portrait, use F11 twice, move between display scales, and close with Alt+F4. Confirm no scrolling, overlap or stretched reset circle. No live Empty Epsilon connection is implemented or claimed.
