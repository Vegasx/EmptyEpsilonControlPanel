const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const dir = path.resolve(process.argv[2]);
const read = file => JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8').replace(/^\uFEFF/, ''));
const config = read('runs.json');
const hardware = read('hardware.json');
const activeAdapters = { cpp: new Set(), wpf: new Set() };
const trials = [];
let scale;
for (const run of config.runs) {
  const report = read(run.report), gpu = read(run.samples);
  assert.equal(report.width, 1000); assert.equal(report.height, 700);
  assert.equal(report.reducedMotion, false);
  scale ??= report.deviceScaleFactor; assert.equal(report.deviceScaleFactor, scale);
  assert.equal(gpu.pid, run.pid); assert.equal(gpu.counterType, 0x20510500, 'Must use PERF_100NSEC_TIMER raw GPU active time');
  assert.equal(gpu.typeReadStatus, 0); assert.equal(gpu.counterResets, 0);
  assert.ok(gpu.samples.length > 80, 'Must have sustained target-process samples');
  let previous = 0;
  for (const sample of gpu.samples) {
    assert.ok(sample.validEngines > 0);
    assert.ok(sample.endMs > sample.startMs && sample.startMs >= previous - .01, 'Sample intervals must not overlap');
    previous = sample.endMs;
    const dt = Number(BigInt(sample.elapsed100ns)) / 10000;
    assert.ok(Math.abs(dt - (sample.endMs - sample.startMs)) < .001);
    for (const engine of sample.active) {
      assert.ok(engine.instance.startsWith(`pid_${run.pid}_`), 'Must not charge another process');
      const luid = engine.instance.match(/_(luid_0x[0-9a-f]+_0x[0-9a-f]+)_phys_/i)?.[1].toLowerCase();
      assert.ok(hardware.adapters.some(a => a.luid === luid && !a.software), 'Must identify a hardware GPU for active engine samples');
      activeAdapters[run.implementation].add(luid);
      const busy = Number(BigInt(engine.busy100ns)) / 10000;
      assert.ok(Math.abs(busy - engine.busyMs) < 1e-9);
      assert.ok(Math.abs(100 * busy / dt - engine.percent) < 1e-9);
      assert.ok(engine.percent >= 0 && engine.percent <= 100.01, 'Invalid single-engine utilization');
    }
  }
  for (const phase of report.phases) {
    assert.ok(phase.pids.some(p => p.id === run.pid), 'Phase must belong to sampled PID');
    const start = Date.parse(phase.start), end = Date.parse(phase.end);
    assert.ok(Number.isFinite(start) && end > start);
    assert.ok(Math.abs(end - start - phase.elapsedMs) < 100, 'UTC/monotonic phase durations must agree');
    if (phase.name === 'switching') { assert.equal(phase.actions, 120); assert.equal(phase.resets, 17); }
    if (phase.name === 'dragging') assert.equal(phase.actions, 300);
    const samples = gpu.samples.filter(s => s.startMs >= start && s.endMs <= end);
    const coverageMs = samples.reduce((sum, s) => sum + Number(BigInt(s.elapsed100ns)) / 10000, 0);
    assert.ok(coverageMs / phase.elapsedMs >= .8, `Insufficient coverage: ${run.implementation} ${phase.name}: ${coverageMs} / ${phase.elapsedMs}`);
    const engines = new Map();
    let weightedMax = 0;
    for (const sample of samples) {
      const dt = Number(BigInt(sample.elapsed100ns)) / 10000;
      weightedMax += Math.max(0, ...sample.active.map(e => e.percent)) / 100 * dt;
      for (const engine of sample.active) {
        const name = engine.instance.replace(/^pid_\d+_/, '');
        engines.set(name, (engines.get(name) || 0) + engine.busyMs);
      }
    }
    const busyMs = [...engines.values()].reduce((sum, ms) => sum + ms, 0);
    trials.push({ implementation: run.implementation, trial: run.trial, phase: phase.name,
      phaseElapsedMs: phase.elapsedMs, coverageMs, coveragePercent: 100 * coverageMs / phase.elapsedMs,
      samples: samples.length, busyMs, combinedEngineUtilPercent: 100 * busyMs / coverageMs,
      busiestEngineUtilPercent: 100 * weightedMax / coverageMs, engines: Object.fromEntries(engines),
      invalidCounterReads: gpu.invalid, source: run.samples });
  }
}
const aggregate = [];
assert.ok(activeAdapters.cpp.size > 0 && activeAdapters.wpf.size > 0, 'Both apps must have measured hardware GPU activity');
assert.deepEqual([...activeAdapters.cpp].sort(), [...activeAdapters.wpf].sort(), 'Both apps must use the same GPU adapters');
for (const phase of ['idle', 'switching', 'settled', 'dragging']) {
  for (const implementation of ['cpp', 'wpf']) {
    const selected = trials.filter(t => t.phase === phase && t.implementation === implementation);
    assert.ok(selected.length >= 2, 'Need at least two reversed-order trials');
    const coverage = selected.reduce((sum, t) => sum + t.coverageMs, 0);
    const busy = selected.reduce((sum, t) => sum + t.busyMs, 0);
    aggregate.push({ implementation, phase, trials: selected.length,
      meanBusyMs: busy / selected.length, meanCoverageMs: coverage / selected.length,
      combinedEngineUtilPercent: 100 * busy / coverage,
      busiestEngineUtilPercent: selected.reduce((sum, t) => sum + t.busiestEngineUtilPercent * t.coverageMs, 0) / coverage,
      busyMsPerSecond: 1000 * busy / coverage,
      individualBusyMsPerSecond: selected.map(t => t.busyMs / t.coverageMs * 1000) });
  }
}
const summary = { generated: new Date().toISOString(), units: 'Summed GPU engine active time; not display latency or per-frame execution time',
  deviceScaleFactor: scale, hardware, activeAdapters: Object.fromEntries(Object.entries(activeAdapters).map(([name, set]) => [name, [...set]])), validationPassed: true, aggregate, trials };
fs.writeFileSync(path.join(dir, 'comparison-gpu.json'), JSON.stringify(summary, null, 2));
console.table(aggregate.map(a => ({ implementation: a.implementation, phase: a.phase,
  meanBusyMs: a.meanBusyMs.toFixed(3), coverageSeconds: (a.meanCoverageMs / 1000).toFixed(3),
  combinedEnginePercent: a.combinedEngineUtilPercent.toFixed(4), busyMsPerSecond: a.busyMsPerSecond.toFixed(3) })));
