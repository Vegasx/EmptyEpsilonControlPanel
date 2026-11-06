import { Controller, systems, getCoolantAllocation } from './model.mjs';
import { LocalAdapter } from './adapter.mjs';
const $ = selector => document.querySelector(selector);
let controller;
const rows = new Map();
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
let renderedMode = null;
let animateNextRender = false;

function setTransforms(record, value) {
  record.handle.style.transform = 'translate3d(' + (value / 300 * record.travelWidth) + 'px,-50%,0) translateX(-50%)';
  record.fill.style.transform = 'scaleX(' + Math.max(0, Math.min(1, value / 300)) + ')';
}
function paintPosition(record, value) {
  record.displayed = value;
  setTransforms(record, value);
}
function sampleMotion(motion, elapsed) {
  let start = motion.from;
  for (const segment of motion.segments) {
    if (elapsed < segment.duration) return start + (segment.to - start) * Math.max(0, elapsed) / segment.duration;
    elapsed -= segment.duration;
    start = segment.to;
  }
  return motion.target;
}
function cancelMotion(record) {
  if (record.motion) record.displayed = sampleMotion(record.motion, record.animations[0].currentTime ?? 0);
  record.motion = null;
  for (const animation of record.animations) animation.cancel();
  record.animations = [];
  record.row.classList.remove('moving');
  paintPosition(record, record.displayed);
}
function finishMotion(record) {
  cancelMotion(record);
  if (record.target !== undefined) paintPosition(record, record.target);
}
function animatePosition(record, target) {
  cancelMotion(record);
  const from = record.displayed;
  const delta = target - from;
  const distance = Math.abs(delta);
  if (reducedMotion.matches || distance < 0.01 || record.travelWidth === 0) { paintPosition(record, target); return; }
  const segments = [
    { to: target + delta * 0.1, duration: Math.min(distance, 300) / 300 * 170 },
    { to: target - delta * 0.01, duration: 24 },
    { to: target + delta * 0.001, duration: 20 },
    { to: target - delta * 0.0001, duration: 18 },
    { to: target, duration: 18 }
  ];
  const duration = segments.reduce((sum, segment) => sum + segment.duration, 0);
  const points = [{ value: from, offset: 0 }];
  let elapsed = 0, previous = from;
  for (const segment of segments) {
    // Preserve exact clamping when the decorative handle passes an endpoint.
    const crossings = [0, 300].map(boundary => ({boundary, t:(boundary - previous) / (segment.to - previous)}))
      .filter(crossing => crossing.t > 0 && crossing.t < 1).sort((a,b) => a.t-b.t);
    for (const crossing of crossings) points.push({value:crossing.boundary, offset:(elapsed+segment.duration*crossing.t)/duration});
    elapsed += segment.duration;
    points.push({value:segment.to, offset:elapsed/duration});
    previous = segment.to;
  }
  const handleFrames = points.map(point => ({offset:point.offset, transform:'translate3d(' + (point.value/300*record.travelWidth) + 'px,-50%,0) translateX(-50%)'}));
  const fillFrames = points.map(point => ({offset:point.offset, transform:'scaleX(' + Math.max(0,Math.min(1,point.value/300)) + ')'}));
  const motion = {from, target, segments};
  record.motion = motion;
  record.row.classList.add('moving');
  // Underlying transforms hold the final position when the compositor finishes.
  setTransforms(record, target);
  record.animations = [record.handle.animate(handleFrames, {duration,easing:'linear'}), record.fill.animate(fillFrames, {duration,easing:'linear'})];
  Promise.all(record.animations.map(animation => animation.finished)).then(() => {
    if (record.motion !== motion) return;
    record.motion = null;
    record.animations = [];
    paintPosition(record, target);
    record.row.classList.remove('moving');
  }).catch(() => { /* Interrupted animations are sampled before cancellation. */ });
}
const sizeObserver = new ResizeObserver(entries => {
  for (const entry of entries) {
    const record = pathRecords.get(entry.target);
    const moving = record.motion !== null;
    cancelMotion(record);
    record.travelWidth = entry.contentRect.width;
    if (moving) animatePosition(record, record.target);
    else paintPosition(record, record.target ?? record.displayed);
  }
});
const pathRecords = new Map();
for (const system of systems) {
  const row = document.createElement('div');
  row.className = 'system-row';
  row.innerHTML = `<div class="system-name"><label for="${system.id}">${system.label}</label></div><div class="range-shell"><div class="track"><div class="track-fill"></div></div><div class="handle-path" aria-hidden="true"><span class="visual-handle"></span></div><input type="range" id="${system.id}" min="0" max="300" step="1" value="0" disabled></div><output class="readout" for="${system.id}"><span class="readout-value">—</span><small>%</small></output>`;
  const input = row.querySelector('input');
  let keyboardEditing = false;
  input.addEventListener('pointerdown', () => { keyboardEditing = false; finishMotion(rows.get(system.id)); });
  input.addEventListener('keydown', event => {
    if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(event.key)) { keyboardEditing = true; finishMotion(rows.get(system.id)); }
  });
  input.addEventListener('input', () => controller.setLevel(system.id, Number(input.value)));
  input.addEventListener('change', () => {
    if (!keyboardEditing) controller.commitLevel(system.id, Number(input.value));
  });
  // Delay keyboard snapping until focus leaves, so one-point arrows can escape a stop.
  input.addEventListener('blur', () => {
    if (keyboardEditing) controller.commitLevel(system.id, Number(input.value));
    keyboardEditing = false;
  });
  $('#systems').append(row);
  const record = {row, input, valueNode:row.querySelector('.readout-value'), handle:row.querySelector('.visual-handle'), fill:row.querySelector('.track-fill'), path:row.querySelector('.handle-path'), displayed:0, target:undefined, percent:undefined, travelWidth:0, motion:null, animations:[]};
  rows.set(system.id, record);
  pathRecords.set(record.path, record);
  sizeObserver.observe(record.path);
}
function feedback(message, error = false) {
  $('#feedback').textContent = message;
  $('#feedback').className = error ? 'error-banner' : 'sr-only';
}
const consolePanel = $('.console');
const modeSwitch = $('#mode-switch');
const powerLabel = $('#power-label');
const coolantLabel = $('#coolant-label');
const resetButton = $('#reset');
const scaleTicks = [...document.querySelectorAll('.scale span')];
function render() {
  const {mode, levels} = controller;
  const modeChanged = renderedMode !== mode;
  const animate = (renderedMode !== null && modeChanged) || animateNextRender;
  animateNextRender = false;
  const allocation = mode === 'coolant' ? getCoolantAllocation(levels.coolant) : null;
  const scaleMax = allocation ? allocation.scaleMax : 300;
  scaleTicks.forEach((tick,index) => {
    const text = Math.round(scaleMax*index/3)+'%';
    if (tick.textContent !== text) tick.textContent = text;
  });
  if (modeChanged) {
    consolePanel.dataset.mode = mode;
    modeSwitch.setAttribute('aria-checked', String(mode === 'coolant'));
    powerLabel.classList.toggle('selected', mode === 'power');
    coolantLabel.classList.toggle('selected', mode === 'coolant');
  }
  for (const {id,label} of systems) {
    const record = rows.get(id);
    const {row,input,valueNode} = record;
    const value = levels[mode][id];
    const changed = record.target !== value;
    const percent = allocation ? Math.round(allocation.shares[id]) : value;
    if (changed || modeChanged) input.value = value;
    if (record.target === undefined) input.disabled = false;
    if (modeChanged) input.setAttribute('aria-label', label+' '+mode);
    if (record.percent !== percent || modeChanged) {
      valueNode.textContent = percent;
      input.setAttribute('aria-valuetext', percent+(allocation ? ' percent of coolant' : ' percent power'));
      record.percent = percent;
    }
    record.target = value;
    if (animate) animatePosition(record,value);
    else if (changed || renderedMode === null) finishMotion(record);
    if (changed || modeChanged) row.classList.toggle('overdrive', mode === 'power' && value>100);
  }
  renderedMode = mode;
  resetButton.disabled = false;
  modeSwitch.disabled = false;
}
async function bundledConfig() {
  const response = await fetch('./config.json', {cache: 'no-store'});
  if (!response.ok) throw new Error(`Cannot read config.json (${response.status}).`);
  return response.json();
}
function applyConfig(config, source) {
  if (controller) controller.loadConfig(config);
  else {
    controller = new Controller(config, new LocalAdapter());
    controller.addEventListener('change', render);
  }
  render();
  feedback('Practice mode');
}
$('#mode-switch').addEventListener('click', () => {
  controller.setMode(controller.mode === 'power' ? 'coolant' : 'power');
  feedback(`${controller.mode === 'power' ? 'Power' : 'Coolant'} mode`);
});
$('#reset').addEventListener('click', () => {
  animateNextRender = true;
  controller.reset();
  feedback(`${controller.mode === 'power' ? 'Power' : 'Coolant'} levels reset.`);
});
reducedMotion.addEventListener('change', () => {
  if (reducedMotion.matches) for (const record of rows.values()) finishMotion(record);
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) for (const record of rows.values()) finishMotion(record);
});
try { applyConfig(await bundledConfig()); }
catch (error) { feedback(`Configuration error: ${error.message} Edit config.json and reload.`, true); }
