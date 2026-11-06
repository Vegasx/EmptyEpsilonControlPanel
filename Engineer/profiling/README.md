# Controller performance attribution

The inline treemap compares matched native WPF and Chrome workloads. This directory contains the collection tools, analysis, and captured data used to build it. The browser production files are unchanged. Native changes add an explicit `--profile <report.json>` diagnostic launch mode; normal launches retain their existing behavior. The installed self-contained distribution was not replaced.

## Measurements

- **CPU:** Windows ETW sampled executing instructions, nominally every 1 ms, attributed exclusively to the sampled module/method and process role. Nested call stacks are not added together. Unresolved addresses remain visible. The CPU headers use process CPU-time deltas; tile sizes use samples. These are independent estimates and can differ, particularly for very small idle workloads.
- **GPU:** Windows `GPU Engine` utilization counters, matched by PID, adapter, and engine. Instance wildcards are refreshed for every pair of samples so newly launched processes are included. Only complete sample intervals inside a workload phase are included. Busy milliseconds equal utilization times the observed interval; concurrent engine time can overlap and is not total wall time. The visible sampling coverage is the actual observed duration, not the whole phase. Zero below counter resolution is not proof of zero work. Desktop Window Manager and unrelated apps are excluded; their downstream composition cost cannot be cleanly assigned to a single app here. These counters cannot attribute GPU work to individual sliders or CSS properties.
- **Chrome main thread:** a separate detailed Chrome diagnostic pass. The deepest active nested event owns each interval, preventing nested event double counting. Values are elapsed traced task time, not CPU samples or GPU execution time. Categories are inferred from trace event names. Detailed tracing adds overhead, so these durations are not used in the matched CPU/GPU totals.

CPU categories are module-based classifications, not proof of a particular UI feature causing the work. `Graphics and driver CPU` is CPU executing rendering/driver code, not GPU time. Native framework code may resolve to a module without public method symbols. Inspect the raw attribution for the exact names. Samples and timing results include profiler overhead and normal background-system variation.

Interrupt/DPC samples flagged by ETW as non-process work are excluded rather than charged to the interrupted controller. GPU totals come from one additional matched pair with neither CPU sampling nor detailed Chrome tracing enabled. This is a small local diagnostic sample, not a statistically precise GPU benchmark.

## Workload

Each paired run uses a 1000 × 700 content panel, matched device scale, enabled animation, and the same configuration. Native's temporary FPS overlay is disabled only in profiling mode. Chrome is headful in app mode with an isolated automation profile. Two trials reverse the implementation order (native/browser, then browser/native). There is no overlap between the two target apps.

Before measurement, both exercise 30 mode switches, four resets, and 60 slider updates, followed by six seconds of settling. The measured phases are five seconds idle, 120 switches at 10 Hz with reset every seventh switch, five seconds settled idle, and 300 reactor updates at 30 Hz following the same sine wave. The last phase exercises the application's slider update path; it does not inject physical mouse/touch input, measure hit testing, or measure input latency. There is deliberately no permanent frame-rate probe.

Chrome CPU includes its own browser, renderer, GPU, and utility processes. Node's static-file server and the collection harness are excluded. Native process CPU includes its runtime, UI, rendering, and GC work. No energy, temperature, display presentation, or input-to-photon measurements are claimed. Two runs describe this local setup, not a universal browser/native performance ratio.

## Reproduce

Use PowerShell in the project root. Build native first with `Engineer\native-windows\build.ps1 -SkipPublish`. Restore/build `Engineer\profiling\TraceReader\TraceReader.csproj` with the project's bundled .NET SDK and `Engineer\native-windows\NuGet.Config`. Its Microsoft TraceEvent dependency is pinned in the project file; package caches and build outputs are ignored.

Set `PLAYWRIGHT_MODULE` to the installed Playwright package directory and, if needed, `CHROME_EXE` to Chrome's executable. The collector currently defaults Playwright to the bundled Codex runtime path on this computer. Start the ordinary Chrome installation first if relying on automatic executable discovery.

```powershell
Engineer\profiling\run-profile.ps1 -NoBrowserTrace
Engineer\profiling\run-gpu.ps1 -OutputDirectory <new-run-directory>
node Engineer\profiling\summarize.cjs <new-run-directory> <detailed-trace-run-directory>
```

Omit `-NoBrowserTrace` for a separate detailed Chrome trace pass. The kernel sampler requests Windows administrator elevation when needed, uses a uniquely named ETW session, and stops in `finally`; it also has a six-minute failsafe. It does not take over an existing WPR session or alter GPU/browser power settings. The two target windows appear sequentially during collection. Leave them visible and avoid interacting with them or running other benchmarks until collection finishes.

Raw ETL and detailed Chrome traces are large, contain broader system/process metadata, and are ignored by Git. The report and inline view contain only the selected target process data. Incomplete harness-development runs are excluded from the final comparison.

## Sources

- [Microsoft TraceEvent](https://github.com/microsoft/perfview/tree/main/src/TraceEvent) for ETW collection and decoding.
- [Chrome Performance reference](https://developer.chrome.com/docs/devtools/performance/reference) for interpreting main-thread and GPU-process trace activity.
- [Microsoft GPU timing guidance](https://learn.microsoft.com/en-us/windows/win32/direct3dtools/pix/articles/timing-captures/pix-timing-captures) for distinguishing CPU work and GPU engine utilization.
