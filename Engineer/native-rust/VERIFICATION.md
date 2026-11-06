# Rust controller verification

Built November 6, 2026 as a native Windows x64 Rust executable. No performance benchmark or profiling workload was run. No comparative CPU, GPU, memory, frame-pacing or latency claim is made.

## Passed

- Pinned Rust 1.91.0 release build, optimization level 3, thin LTO, one codegen unit, static compiler runtime and warnings treated as errors.
- Four core test groups. Numerical parity uses 2,001 input positions (both rounding and snapping), 2,107 motion samples across seven travel cases, and 512 coolant allocations, generated independently from the optimized C++ model. Additional checks cover independent profiles, exact unsnapped reset defaults, JSON payloads, UTF-8/BOM/UTF-16 name length, malformed/duplicate/oversized/deep JSON, endpoint validation and send-rate limits.
- 27 native window/control/rendering checks, including real HWND keyboard/mouse message dispatch, capture/release, focus-loss snap, rocker activation, fullscreen entry/exit, independent banks, coolant ruler, active-profile Reset, repeated Reset clock preservation, unchanged-slider stability, no redraw on a settled Reset, final-state submission, sleeping idle clock, partial rendering, cache reuse and render-target replacement.
- 15 partial-versus-full image comparisons at 96, 144 and 192 DPI. Cases cover both endpoint overshoots, changing colors, ruler/readout/status restoration and keyboard focus. Maximum permitted per-channel rounding difference: 2/255. All passed.
- Four existing optimized C++ reference screenshots matched the Rust outputs **pixel for pixel**, with zero different channels: 1000×700 power, 640×480 power, 768×1024 portrait power, and 768×1024 portrait coolant. `compare-reference.ps1` reads existing reference images; it does not execute the C++ application.
- Real loopback HTTP delivery against normal and slow/failing receivers: one request in flight, coalesced intermediate states, monotonic sequences and retried final snapped reactor value 150 at sequence 302. The included demo receiver passed as well.
- Explicitly missing config/transport files, invalid endpoint and empty endpoint produce error reports and failure exit codes. An enabled transport configuration caused zero requests during a diagnostic render.
- Portable executable imports Windows system libraries only; the compiler's unwind runtime is linked statically. Font artwork and font data are embedded. Distribution includes editable configuration and third-party license notices.
- The final ZIP was extracted to a separate folder and launched successfully with only the Windows System32 directory on PATH. It rendered its diagnostic image and exited successfully without access to the development compiler/runtime directories.

Evidence: [functional report](artifacts/verification.json), [UI checks](artifacts/ui-verification.json), [C++ image comparison](artifacts/cpp-visual-comparison.json), [portable launch](artifacts/portable-smoke.json), [power](artifacts/rust-panel.png), [minimum](artifacts/rust-panel-minimum.png), [portrait](artifacts/rust-panel-portrait.png), [coolant](artifacts/rust-panel-coolant.png).

## Acceptance still needed

Physical tablet touch/pen operation, multi-monitor DPI changes, remote-display fallback, actual driver/device loss and live server/game integration remain unverified. Custom screen-reader slider providers and simultaneous multi-finger editing are outside the optimized C++ prototype's scope and this port's scope. The optional `--profile` compatibility mode compiles but was deliberately not executed; independent performance testing belongs to the user.

Normal launches use no workload timer, frame sampling or FPS overlay. The other controller implementations were not edited by this work.
