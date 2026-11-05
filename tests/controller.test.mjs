import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Controller, systems, validateConfig, snapLevel} from '../model.mjs';
const config = JSON.parse(await readFile(new URL('../config.json', import.meta.url)));
test('power and coolant are independent and accept full range', () => {
  const controller = new Controller(config);
  for (const {id} of systems) {
    controller.setLevel(id, 300);
    assert.equal(controller.levels.power[id], 300);
    assert.equal(controller.levels.coolant[id], config.defaults.coolant[id]);
  }
  controller.setMode('coolant');
  for (const {id} of systems) controller.setLevel(id, 0);
  controller.setMode('power');
  for (const {id} of systems) assert.equal(controller.levels.power[id], 300);
  controller.reset();
  assert.deepEqual(controller.levels.power, config.defaults.power);
  for (const {id} of systems) assert.equal(controller.levels.coolant[id], 0);
});
test('custom JSON defaults apply to both banks and invalid imports leave state intact', () => {
  const controller = new Controller(config);
  const custom = structuredClone(config);
  custom.defaults.power.reactor = 238;
  custom.defaults.coolant.warp = 191;
  controller.loadConfig(custom);
  controller.setLevel('reactor', 7);
  controller.reset();
  assert.equal(controller.levels.power.reactor, 238);
  assert.equal(controller.levels.coolant.warp, 191);
  const before = structuredClone(controller.levels);
  custom.defaults.coolant.warp = 301;
  assert.throws(() => controller.loadConfig(custom));
  assert.deepEqual(controller.levels, before);
});
test('validation rejects missing, fractional and nonnumeric defaults', () => {
  for (const bad of [undefined, null, '100', -1, 301, 100.5]) {
    const copy = structuredClone(config);
    copy.defaults.power.reactor = bad;
    assert.throws(() => validateConfig(copy));
  }
});
test('adapter receives isolated complete percentage snapshots', () => {
  let received;
  const controller = new Controller(config, {applyLevels(levels) { received = levels; }});
  controller.setLevel('reactor', 280);
  assert.equal(received.power.reactor, 280);
  assert.equal(received.coolant.reactor, config.defaults.coolant.reactor);
  received.power.reactor = 0;
  assert.equal(controller.levels.power.reactor, 280);
  controller.reset();
  assert.deepEqual(received, config.defaults);
});


test('light snapping attracts adjacent values to multiples of five and ten', () => {
  for (const [input, expected] of [[0,0],[1,0],[2,2],[3,3],[4,5],[6,5],[7,7],[8,8],[9,10],[11,10],[14,15],[16,15],[19,20],[21,20],[84,85],[86,85],[89,90],[91,90],[96,95],[104,105],[146,145],[154,155],[294,295],[296,295]]) {
    assert.equal(snapLevel(input), expected, String(input));
  }
});
test('major stops have inclusive three-point attraction in either direction', () => {
  for (const stop of [100,150,200,250,300]) {
    for (let delta=-3;delta<=3;delta++) {
      if (stop+delta<=300) assert.equal(snapLevel(stop+delta),stop, String(stop+delta));
    }
  }
});
test('snapping stays in range and is stable after repeated commits', () => {
  for (let value=-10;value<=310;value++) {
    const snapped=snapLevel(value);
    assert.ok(snapped>=0 && snapped<=300);
    assert.equal(snapLevel(snapped),snapped);
  }
  assert.throws(()=>snapLevel(NaN));
});
test('drag values stay exact until release; reset preserves arbitrary configured defaults', () => {
  const custom=structuredClone(config);
  custom.defaults.power.reactor=99;
  custom.defaults.coolant.reactor=149;
  const controller=new Controller(custom);
  controller.setLevel('reactor',148);
  assert.equal(controller.levels.power.reactor,148);
  controller.commitLevel('reactor');
  assert.equal(controller.levels.power.reactor,150);
  controller.setMode('coolant');
  controller.setLevel('reactor',199);
  controller.commitLevel('reactor');
  assert.equal(controller.levels.coolant.reactor,200);
  assert.equal(controller.levels.power.reactor,150);
  controller.reset();
  assert.equal(controller.levels.power.reactor,150);
  assert.equal(controller.levels.coolant.reactor,149);
});

for (const mode of ['power', 'coolant']) {
  test(`reset restores all ${mode} defaults and preserves every slider in the other profile`, () => {
    let sent;
    const controller = new Controller(config, {applyLevels(levels) { sent = levels; }});
    for (const profile of ['power','coolant']) {
      controller.setMode(profile);
      for (const [index,{id}] of systems.entries()) controller.setLevel(id, 170+index);
    }
    const other = mode === 'power' ? 'coolant' : 'power';
    const preserved = structuredClone(controller.levels[other]);
    controller.setMode(mode);
    controller.reset();
    assert.equal(controller.mode, mode);
    assert.deepEqual(controller.levels[mode], config.defaults[mode]);
    assert.deepEqual(controller.levels[other], preserved);
    assert.deepEqual(sent[other], preserved);
    assert.deepEqual(sent[mode], config.defaults[mode]);
  });
}