import { Controller, systems, getCoolantAllocation } from './model.mjs';
import { LocalAdapter } from './adapter.mjs';
const $ = selector => document.querySelector(selector);
let controller;
const rows = new Map();
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
let renderedMode = null;
let animateNextRender = false;

function paintPosition(record, value) {
  record.displayed = value;
  record.row.style.setProperty('--level', (Math.max(0, Math.min(300, value)) / 3) + '%');
  record.row.style.setProperty('--handle-level', (value / 3) + '%');
}
function cancelMotion(record) {
  cancelAnimationFrame(record.frame);
  record.frame = null;
  record.row.classList.remove('moving');
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
  if (reducedMotion.matches || distance < 0.01) { paintPosition(record, target); return; }
  // Linear travel reaches 10% beyond the target, then each correction leaves
  // an opposite error one tenth as large: +10%, -1%, +0.1%, -0.01%, exact.
  // Four end corrections get real frame time, within a 250ms full-range move.
  const segments = [
    { to: target + delta * 0.1, duration: Math.min(distance, 300) / 300 * 170 },
    { to: target - delta * 0.01, duration: 24 },
    { to: target + delta * 0.001, duration: 20 },
    { to: target - delta * 0.0001, duration: 18 },
    { to: target, duration: 18 }
  ];
  const duration = segments.reduce((sum, segment) => sum + segment.duration, 0);
  const started = performance.now();
  record.row.classList.add('moving');
  function tick(now) {
    const elapsed = Math.max(0, now - started);
    if (elapsed >= duration) {
      paintPosition(record, target);
      record.frame = null;
      record.row.classList.remove('moving');
      return;
    }
    let remaining = elapsed;
    let start = from;
    for (const segment of segments) {
      if (remaining < segment.duration) {
        paintPosition(record, start + (segment.to - start) * remaining / segment.duration);
        break;
      }
      remaining -= segment.duration;
      start = segment.to;
    }
    record.frame = requestAnimationFrame(tick);
  }
  record.frame = requestAnimationFrame(tick);
}
for (const system of systems) {
  const row = document.createElement('div');
  row.className = 'system-row';
  row.innerHTML = `<div class="system-name"><label for="${system.id}">${system.label}</label></div><div class="range-shell"><div class="track"><div class="track-fill"></div></div><div class="handle-path" aria-hidden="true"><span class="visual-handle"></span></div><input type="range" id="${system.id}" min="0" max="300" step="1" value="0" disabled></div><output class="readout" for="${system.id}">—</output>`;
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
  rows.set(system.id, { row, input, output: row.querySelector('output'), displayed: 0, target: undefined, frame: null });
}
function feedback(message, error = false) {
  $('#feedback').textContent = message;
  $('#feedback').className = error ? 'error-banner' : 'sr-only';
}
function render() {
  const { mode, levels } = controller;
  const switching = renderedMode !== null && renderedMode !== mode;
  const animate = switching || animateNextRender;
  animateNextRender = false;
  const allocation = mode === 'coolant' ? getCoolantAllocation(levels.coolant) : null;
  const scaleMax = allocation ? allocation.scaleMax : 300;
  document.querySelectorAll('.scale span').forEach((tick, index) => {
    tick.textContent = Math.round(scaleMax * index / 3) + '%';
  });
  $('.console').dataset.mode = mode;
  $('#mode-switch').setAttribute('aria-checked', String(mode === 'coolant'));
  $('#power-label').classList.toggle('selected', mode === 'power');
  $('#coolant-label').classList.toggle('selected', mode === 'coolant');
  for (const {id, label} of systems) {
    const record = rows.get(id);
    const {row, input, output} = record;
    const value = levels[mode][id];
    input.value = value;
    input.disabled = false;
    input.setAttribute('aria-label', `${label} ${mode}`);
    const percent = allocation ? Math.round(allocation.shares[id]) : value;
    input.setAttribute('aria-valuetext', allocation ? percent + ' percent of coolant' : percent + ' percent power');
    output.innerHTML = percent + '<small>%</small>';
    const changed = record.target !== value;
    record.target = value;
    if (animate) animatePosition(record, value);
    else if (changed || renderedMode === null) finishMotion(record);
    row.classList.toggle('overdrive', mode === 'power' && value > 100);
  }
  renderedMode = mode;
  $('#reset').disabled = false;
  $('#mode-switch').disabled = false;
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
