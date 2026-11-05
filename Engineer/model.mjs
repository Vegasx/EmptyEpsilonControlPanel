export const systems = Object.freeze([
  { id: 'reactor', label: 'Reactor', group: 'Core systems' },
  { id: 'beamweapons', label: 'Beam weapons', group: 'Weapons' },
  { id: 'missilesystem', label: 'Missile system', group: 'Weapons' },
  { id: 'maneuver', label: 'Maneuvering', group: 'Propulsion' },
  { id: 'impulse', label: 'Impulse engines', group: 'Propulsion' },
  { id: 'warp', label: 'Warp drive', group: 'Propulsion' },
  { id: 'frontshield', label: 'Front shields', group: 'Defense' },
  { id: 'rearshield', label: 'Rear shields', group: 'Defense' }
]);
export const modes = ['power', 'coolant'];

// Coolant can leave capacity unused; normalize only when requests exceed one full bar.
// The ruler follows a fully-right slider, capped at 100% for small totals.
export function getCoolantAllocation(weights) {
  const total = systems.reduce((sum, {id}) => sum + weights[id], 0);
  const capacity = Math.max(300, total);
  const shares = Object.fromEntries(systems.map(({id}) => [id, weights[id] / capacity * 100]));
  return { total, shares, scaleMax: 300 / capacity * 100 };
}

// Release-only magnetic stops: major stops take priority over five-point stops.
export function snapLevel(value) {
  if (!Number.isFinite(value)) throw new Error('Level must be a number.');
  const level = Math.max(0, Math.min(300, Math.round(value)));
  for (const stop of [100, 150, 200, 250, 300]) {
    if (Math.abs(level - stop) <= 3) return stop;
  }
  const nearestFive = Math.round(level / 5) * 5;
  return Math.abs(level - nearestFive) <= 1 ? nearestFive : level;
}
export function validateConfig(config) {
  if (!config || config.version !== 1) throw new Error('Configuration version must be 1.');
  if (typeof config.shipName !== 'string' || !config.shipName.trim() || config.shipName.length > 60) throw new Error('Ship name must contain 1–60 characters.');
  for (const mode of modes) for (const {id} of systems) {
    const value = config.defaults?.[mode]?.[id];
    if (!Number.isInteger(value) || value < 0 || value > 300) throw new Error(`${mode}.${id} must be a whole number from 0 to 300.`);
  }
  return structuredClone(config);
}
export class Controller extends EventTarget {
  constructor(config, adapter) {
    super();
    this.config = validateConfig(config);
    this.adapter = adapter;
    this.mode = 'power';
    this.levels = structuredClone(this.config.defaults);
  }
  setMode(mode) {
    if (!modes.includes(mode)) throw new Error('Unknown mode.');
    this.mode = mode;
    this.dispatchEvent(new Event('change'));
  }
  setLevel(id, value) {
    if (!systems.some(system => system.id === id)) throw new Error('Unknown system.');
    if (!Number.isFinite(value)) throw new Error('Level must be a number.');
    this.levels[this.mode][id] = Math.max(0, Math.min(300, Math.round(value)));
    this.publish();
  }
  commitLevel(id, value = this.levels[this.mode][id]) {
    this.setLevel(id, snapLevel(value));
  }
  reset() {
    this.levels[this.mode] = structuredClone(this.config.defaults[this.mode]);
    this.publish();
  }
  loadConfig(config) {
    this.config = validateConfig(config);
    this.levels = structuredClone(this.config.defaults);
    this.publish();
  }
  publish() {
    this.adapter?.applyLevels(structuredClone(this.levels));
    this.dispatchEvent(new Event('change'));
  }
}
