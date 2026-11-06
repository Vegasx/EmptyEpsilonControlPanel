# Optimized version verification

This is a separate build for independent performance profiling. No comparative CPU/GPU performance result is claimed for it.

## Passed checks

- Optimized C++20 Release build, warnings treated as errors: no warnings or errors.
- **9,938 unchanged core assertions**: slider bounds/snapping, coolant allocation, independent profiles, exact defaults, JSON validation/serialization and animation geometry.
- **71 functional/rendering checks**, including the original controls/layout tests, real HWND partial paints during switching, bounded update area, repeated Reset clock preservation, unchanged sliders remaining static, zero redraws for a settled Reset, final transport submission and the frame scheduler sleeping at idle.
- **15 partial-versus-full image comparisons**: five cases at each of 96, 144 and 192 DPI (100%, 150%, 200%). Cases include old/new grip restoration, overshoot at both ends, all-row color transitions, changed ruler/readouts/status and keyboard focus. Partial pixels matched a separately rebuilt full reference with a maximum permitted per-channel rounding tolerance of 2/255.
- Background bitmap reuse and text/reset opacity-mask reuse were checked without rebuilding those resources for each color frame.
- DXGI vblank delivery was exercised successfully on this machine; no refresh-interval fallback was used in the functional run. Monitor changes and remote/headless fallback still need device acceptance testing.
- Real loopback HTTP tests passed for normal delivery, coalescing, one request in flight, slow/failing receivers, final snapped state retry and the supplied demo receiver.
- Invalid endpoint and explicitly missing configuration/transport files fail with diagnostic reports. Diagnostic runs disable configured server delivery.
- Normal, 640×480 minimum, portrait and coolant render images were generated; representative layouts were visually inspected.

Evidence: [verification.json](artifacts/verification.json), [UI/render checks](artifacts/ui-verification.json), [normal view](artifacts/cpp-panel.png), [minimum size](artifacts/cpp-panel-minimum.png), [coolant portrait](artifacts/cpp-panel-coolant.png), and [baseline preservation](artifacts/baseline-preservation.json).

The baseline main source, model/transport headers, executable and ZIP were SHA-256 checked against baseline-hashes.json and remain unchanged. The optimized model, transport and core-test files are byte-identical to the baseline copies. Animation durations, overshoot and piecewise-linear interpolation are preserved; repeated Reset intentionally no longer restarts an animation already headed to its requested default.

The rendering checks exercise real Win32 message handling and the shared drawing path, including offscreen pixel comparisons; they are not physical touch/pen tests, display-present latency measurements or comparative GPU profiling. Please compare the separate applications on the tablet at the same window size, display scale, animation preference and transport settings.
