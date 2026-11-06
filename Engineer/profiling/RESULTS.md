# Profiling results — November 5, 2026

The first optimization lead is browser rendering/composition, especially style recalculation during profile-color transitions. This is a profiling finding, not a measured optimization: no production CSS, artwork, animation, or browser logic was changed.

## Matched CPU comparison

Source: `artifacts/run-20261105-194435/comparison.json` and its two native/two browser phase reports. The two runs reversed implementation order, exercised interaction paths before measurement, and used 1000 × 700 panels at device scale 1. This machine exposed 16 logical processors. Native used .NET 10.0.14 and hardware rendering tier 2; the installed Chrome version was 132. The native FPS overlay and Chrome detailed tracing were off. Windows ETW sampled CPU execution; zero ETW events were lost. Interrupt/DPC samples marked as non-process work were excluded.

During 120 switches and 17 resets over approximately 12 seconds per trial:

| Measurement, mean per trial | Native WPF | Chrome |
| --- | ---: | ---: |
| Process CPU time | 1.52 s | 8.30 s |
| Process CPU, total-machine basis | 0.79% | 4.32% |
| ETW estimated executing CPU time | 1.47 s | 7.44 s |

These are instrumented local results, not a reproduction of a universal “40% more CPU” ratio. CPU-time deltas and 1 ms execution samples are separate measurement sources. In the native slider-update phase, process counters reported extremely little CPU while ETW still captured roughly 184 ms per trial. Low-usage results should therefore be treated as approximate rather than precise ratios; the visual retains both measurement sources instead of forcing agreement.

About 60% of Chrome's switching CPU samples were in its **GPU process** (4.43 s/trial), versus 40% in its renderer processes (2.98 s/trial). This is **CPU execution inside those processes**, not time executing on GPU hardware. Native's largest category was graphics/driver CPU (about 52% of its samples). Application model arithmetic was a small directly attributed component.

## Main-thread detail

Source: the separate detailed trace pass at `artifacts/run-20261105-193851`, with the same measured actions and panel dimensions. Its earlier warmup did not exercise interaction paths. Detailed tracing was enabled and affected performance, so its timings are not substituted into the matched CPU comparison.

Exclusive main-thread trace durations during switching averaged approximately 995 ms style/layout, 548 ms paint/composition preparation, 309 ms animation processing, 399 ms other renderer work, and 30 ms directly classified JavaScript/event handling. Style/layout was about 44% of partitioned main-thread trace time; `Document::recalcStyle` alone was about 841 ms. Event-name classification and native module samples describe different levels of the rendering pipeline.

The native interaction-cold diagnostic pass also spent substantial time in the JIT compiler. Warming the interaction paths reduced this component considerably. This is why the final comparison uses the warmed runs.

## GPU engine comparison

Source: `native-gpu.json`, `browser-gpu.json`, `gpu-sources.json`, and ignored raw `gpu-isolated.json` in the final run directory. This is **one additional matched pair**, with neither ETW CPU sampling nor detailed Chrome tracing enabled. Invalid counters from exiting processes were skipped, and instance enumeration was refreshed for each pair of counter reads.

During switching, native's complete observed intervals covered 5.046 s and contained about 50.0 ms GPU-engine busy time (0.99% combined engine utilization). Chrome's covered 4.041 s and contained about 149.5 ms (3.70%). These percentages normalize the unequal coverage. They sum separately observed engines; overlapping hardware engines can contribute simultaneously. They are not a Task Manager busiest-engine percentage or a measurement of shader efficiency, energy, display latency, or total GPU capacity.

The sample is sparse, particularly at idle. Desktop Window Manager and unrelated applications are excluded. The counters identify engines and target processes, not individual CSS rules or visual elements. No claim that color transitions consume a particular fraction of GPU time is established.

## Next experiments

1. A/B test the browser's color transitions while keeping slider movement, timings, dimensions, and workload fixed. The current trace makes repeated style recalculation the clearest app-level hypothesis.
2. Inspect layer invalidations and raster/compositor work, then test the permanent transform layer hints. Retain the existing transform-based movement unless evidence favors a change.
3. Repeat lightweight paired runs after each change, including identical visual output and frame-presentation checks. Lower CPU/GPU utilization alone does not establish better smoothness or input latency.

## Validation

The native Release build completed without warnings or errors, and all 5,815 existing core assertions passed. The opt-in native profile and Chrome workload completed their phases. The analysis checks target dimensions, device scale, enabled animation, zero lost ETW events, CPU tile/sample conservation, and nontrivial GPU sampling coverage. The interactive view passed 18 checks covering all workloads/measurements, tile drill-down, breadcrumb return, exact values, and layouts at desktop and narrow widths. Its rendered desktop view was inspected.

Raw GPU/ETW/Chrome traces are ignored by Git. The interactive view embeds only aggregated target-process results. Collection scripts and methodology are documented in [README.md](README.md).
