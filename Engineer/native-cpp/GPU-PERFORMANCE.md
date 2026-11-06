# GPU comparison — November 5, 2026

The C++ / Direct2D app recorded **about 12.3% less GPU engine busy time per second during profile switching** than the C# / WPF app. Slider updates measured about 4.5% lower, a small absolute difference. Both recorded zero app-attributed GPU activity during the initial idle samples. These are two local, reversed-order comparisons, not a general performance guarantee or a measurement of input/display latency.

## Results

Both applications used the **NVIDIA GeForce RTX 5080**, adapter `luid_0x00000000_0x00017f82`, physical engine 0 / 3D. No other engine contributed nonzero active time for either target during these measured phases. The observed sum across engines therefore equals the busiest-engine result in this capture. [Hardware metadata](artifacts/gpu-20261105-205358/hardware.json) comes from DXGI adapter enumeration; the Microsoft Basic Render Driver was also enumerated but was not the active measured adapter.

| Workload | C++ busy time, mean | WPF busy time, mean | C++ mean GPU use | WPF mean GPU use |
| --- | ---: | ---: | ---: | ---: |
| Repeated profile switching | 26.251 ms | 29.917 ms | 0.2233% | 0.2547% |
| Slider updates at 30 Hz | 3.985 ms | 4.167 ms | 0.0409% | 0.0428% |
| Initial idle | 0 recorded | 0 recorded | 0 recorded | 0 recorded |
| Settling after switching | 0.208 ms | 1.092 ms | 0.0044% | 0.0230% |

Busy-time totals apply to **observed intervals**, not a silently extrapolated full phase. Switching coverage averaged 11.754 s for C++ and 11.748 s for WPF within each approximately 12 s phase. Slider-update coverage averaged 9.748 s and 9.738 s within the approximately 10 s phases. Percentages normalize by actual coverage. The relative reductions above compare those normalized rates, not unequal observation windows.

For switching, the individual trials measured C++ 2.163 / 2.303 ms GPU busy time per observed second, versus WPF 2.574 / 2.519 ms. C++ was lower in both orders. For slider updates the rates were C++ 0.411 / 0.407 ms/s, versus WPF 0.420 / 0.436 ms/s. That difference is only about 0.019 ms of GPU time per observed second on this GPU; treat it as a small measured difference, with more runs/device testing needed to establish practical significance.

The settling phase includes the tail of the last animation. WPF's settling result varied between trials, and the absolute costs were tiny; it is not used to claim a stable idle percentage improvement. Zero idle readings mean no activity recorded by the counter during the observed windows, not proof of zero system/display GPU work.

## Collection and validation

The existing warmed workload was used: 1000×700 DIPs, scale 1.0, animations enabled, no FPS overlay, no HTTP traffic. Warmup exercises 30 switches and 60 slider updates followed by a reset and six seconds settling. Measured phases are five seconds initial idle, 120 switches / 17 resets at 10 Hz over 12 seconds, five seconds settling, and 300 sinusoidal slider updates at 30 Hz over ten seconds. Trial order was C++ then WPF, then WPF then C++. Targets ran sequentially on the same system/GPU. CPU tracing and browser tracing were disabled. Collection includes helper overhead and ordinary background-system variation.

A separate native PDH helper collects `GPU Engine(*) / Utilization Percentage` raw counter arrays every 250 ms. It retains only instances belonging to the target PID, including valid zero-valued instances. Instances are enumerated from the wildcard array each time. The helper verifies `PERF_100NSEC_TIMER` counter type; cumulative active-time differences are retained as integer strings in 100 ns units and converted to busy milliseconds. Utilization is active-time delta / elapsed-time delta × 100. Raw integer values are preserved without JSON floating-point precision loss.

C++ profile reports now include UTC phase start/end and process IDs; this is diagnostic metadata only. WPF's existing profile already supplied both. Only complete counter intervals strictly within each phase are used; boundary-crossing intervals are omitted. Active-phase coverage was 97.27–97.92%, with 47 complete switching intervals and 39 slider-update intervals per trial. Idle/settling coverage was approximately 95%. No invalid counter reads or counter resets were recorded. The sampler tracked 15 valid engine instances per sample for the selected adapter, so an idle zero is distinguished from missing process counters.

The summarizer validates counter type/status, PID attribution, nonoverlapping intervals, raw-to-cooked arithmetic, timestamps versus monotonic phase duration, sampling coverage, equal panel size/scale, enabled animation and workload action counts. All validations passed. The application still passes 9,938 core assertions and the 18 UI checks; HTTP tests were rerun after the diagnostic metadata change.

These are **app-attributed GPU engine busy times**. Desktop Window Manager, other processes, GPU memory, energy, shader-instruction throughput and GPU frequency were not attributed. GPU engines can run concurrently, so summed engine time generally is not unique GPU wall time; this run had only one active engine for the target apps. No per-frame GPU timestamp query, presentation trace, input-to-photon test or physical touch test was performed. Lower GPU utilization does not by itself establish smoother animation. The results apply to this RTX 5080 and workload; another device may behave differently. Microsoft's [GPU measurement explanation](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/) describes engine concurrency and busiest-engine aggregation; [PDH counter documentation](https://learn.microsoft.com/en-us/windows/win32/api/pdh/ns-pdh-pdh_raw_counter) describes raw counter data.

## Evidence and reproduction

- [Validated aggregate and per-trial results](artifacts/gpu-20261105-205358/comparison-gpu.json)
- [Run manifest](artifacts/gpu-20261105-205358/runs.json)
- [C++ trial 1 workload](artifacts/gpu-20261105-205358/cpp-1.json) and [GPU samples](artifacts/gpu-20261105-205358/cpp-1-gpu.json)
- [WPF trial 1 workload](artifacts/gpu-20261105-205358/wpf-1.json) and [GPU samples](artifacts/gpu-20261105-205358/wpf-1-gpu.json)
- [WPF trial 2 workload](artifacts/gpu-20261105-205358/wpf-2.json) and [GPU samples](artifacts/gpu-20261105-205358/wpf-2-gpu.json)
- [C++ trial 2 workload](artifacts/gpu-20261105-205358/cpp-2.json) and [GPU samples](artifacts/gpu-20261105-205358/cpp-2-gpu.json)

From native-cpp in PowerShell, build both implementations if needed, then run `./profile-gpu.ps1 -Trials 2`. The script compiles its PDH sampler and DXGI metadata helper with the project-local compiler. It writes a fresh artifacts/gpu-* folder, stops its own processes on errors, validates the results and writes comparison-gpu.json. No administrator elevation or persistent profiling session was needed here. Keep the panels visible during measurement and avoid other GPU workloads. The helpers remain development tools; the deployed app has no new profiling dependency or permanent sampler.
