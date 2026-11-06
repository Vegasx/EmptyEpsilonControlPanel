# Epsilon Engineering — native Rust Windows controller

Launch **Start Rust Controller.cmd**, or **dist/Epsilon-Engineering-Rust-win-x64/Epsilon Engineering Rust.exe**. Copy the complete distribution folder to another Windows 10/11 x64 computer. The executable embeds the Russo One font and original brass grip artwork. It uses Win32, Direct2D, DirectWrite, WIC, DXGI and WinHTTP directly. No browser, webview, JavaScript, .NET runtime or Rust installation is needed to run it. Node is used only by the optional development verification receiver.

This is an independent Rust implementation of the optimized C++ controller. All application state, rendering, input handling, animation scheduling and HTTP transport are implemented in Rust; no C++ controller code is linked into it. Existing C++, optimized C++ and C# versions are separate and preserved. Performance measurements are left to the user; no faster-than-C++ claim is made.

## Controls and behavior

- Starts fullscreen; F11 toggles a normal resizable window, `--windowed` starts windowed, Alt+F4 closes.
- Eight independent power values, eight independent coolant weights, active-profile Reset, and the same shared coolant allocation/ruler.
- Mouse, single touch or pen dragging. Touch/pen uses Windows pointer messages and implicit capture; simultaneous multi-finger adjustment is outside the optimized C++ reference's scope and this version's scope.
- Tab/Shift+Tab visits the sliders, Reset and rocker. Arrows change one point; Page Up/Down ten; Home/End select 0/300. Space/Enter activates the focused button. Slider edits snap on release or focus loss, using the same major-stop priority as C++.
- Exact configured reset defaults; resetting an already-targeted animation preserves its clock. Switching views changes no transmitted setting.
- Same piecewise-linear travel, distance-based duration up to 250 ms, 10% overshoot and four corrections; 250 ms color and 160 ms rocker transitions. Immediate dragging and Windows reduced-motion preferences are preserved. `--reduced-motion` also disables transitions.
- Per-monitor DPI and resize support; cached static background, bounded tintable A8 masks, separate dirty regions and retained Direct2D presentation. One coalesced DXGI vblank notification; no display polling while settled. A reported-monitor-refresh fallback handles unavailable vblank outputs. Monitor/display changes refresh output selection. Caches rebuild after resize, DPI change or target loss.
- Custom screen-reader slider providers are outside the current reference application's scope.

## Configuration and HTTP

The distribution's editable `config.json` and `transport.json` are preserved by subsequent builds. Defaults are initially copied from the optimized C++ source. Command-line overrides: `--config <path>`, `--transport <path>` and `--endpoint <HTTP/HTTPS URL>`. Configuration validation rejects missing keys, nonintegral/out-of-range values, duplicate JSON keys, malformed Unicode, excessive nesting and files over 1 MiB.

HTTP is disabled by default. Enable it in `transport.json` or pass `--endpoint`. The same version-1 JSON POST contract sends ship name, sequence, final flag, all eight power percentages, all coolant weights and normalized coolant allocations. One background worker owns the synchronous WinHTTP session and connections; it keeps one newest pending full state, limits move sends to `sendHz`, and retries failed final states with bounded exponential backoff. Final sends bypass the move rate limit. Redirects are disabled and HTTP/HTTPS URLs with embedded credentials/fragments are rejected. This is the existing generic server contract; a game-specific Empty Epsilon adapter is not included.

## Build and functional verification

```powershell
.\bootstrap.ps1
.\build.ps1
node .\verify.cjs
```

The pinned Rust 1.91.0 LLVM Windows toolchain lives under `tools`, with no global PATH changes. Build uses the existing project's local LLVM-MinGW resource compiler/linker (bootstrapping it if missing), Cargo.lock, release optimization, thin LTO and one codegen unit. All packages and the resulting executable remain local. `build.ps1 -CheckOnly` checks compilation; `-SkipTests` skips core tests. Font license and Rust dependency licenses accompany the distribution.

Core tests compare rounding, snapping, coolant allocations and motion samples to checked-in values produced independently by `tests/generate-golden.cpp` against the optimized C++ model. That helper is only a development oracle; it is not part of the Rust executable. Regenerate the fixture with the local C++ compiler if the reference rules intentionally change.

See [VERIFICATION.md](VERIFICATION.md) for results and remaining device acceptance. `compare-reference.ps1` compares generated screenshots against the existing optimized C++ reference images without executing a performance workload.

`--verify <report.json>` opens a real native window, checks controls and rendering, writes screenshots/report and closes. `--screenshot <image.png>` writes an offscreen render and closes. `--network-test <endpoint> <report.json>` exercises the real HTTP worker against a development receiver. Diagnostic UI runs disable configured HTTP delivery. Verification does not establish physical touch, screen-reader, tablet, remote-display or live-game acceptance.

The optional `--profile <report.json>` workload is retained for command-line compatibility with optimized C++ and is left for independent use. It has not been executed as part of this development. Normal launches do not allocate frame-interval samples or start a workload timer. Functional verification does not run it.

Windows bindings: [Microsoft windows-rs](https://github.com/microsoft/windows-rs). Renderer references: [Direct2D compatible render targets](https://learn.microsoft.com/en-us/windows/win32/direct2d/id2d1rendertarget-createcompatiblerendertarget), [retained HWND presentation](https://learn.microsoft.com/en-us/windows/win32/api/d2d1/ne-d2d1-d2d1_present_options), [DXGI vblank](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgioutput-waitforvblank). Compiler: [Rust LLVM Windows support](https://doc.rust-lang.org/rustc/platform-support/windows-gnullvm.html).
