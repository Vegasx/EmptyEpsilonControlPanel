// Future Empty Epsilon transport belongs here, separate from touch controls.
// Percentages are UI units. Power maps to EE's 0.0–3.0 via percent / 100.
// Coolant contains raw relative weights: getCoolantAllocation(levels.coolant)
// converts them to 100%-pool shares before per-ship capacity/budget mapping.
// No network requests or simulated ship telemetry are made in this prototype.
export class LocalAdapter {
  applyLevels(levels) { this.lastRequestedLevels = levels; }
}
