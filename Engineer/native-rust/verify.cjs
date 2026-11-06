const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const root = __dirname;
const exe = path.join(root, 'dist/Epsilon-Engineering-Rust-win-x64/Epsilon Engineering Rust.exe');
const artifacts = path.join(root, 'artifacts');
fs.mkdirSync(artifacts, { recursive: true });
function run(args, timeout = 25000) {
  return new Promise((resolve, reject) => {
    const child = spawn(exe, args, { windowsHide: true });
    const timer = setTimeout(() => { child.kill(); reject(new Error('App timed out: ' + args.join(' '))); }, timeout);
    child.on('error', e => { clearTimeout(timer); reject(e); });
    child.on('exit', code => { clearTimeout(timer); resolve(code); });
  });
}
async function receiverTest(name, fail) {
  const received = [];
  let inFlight = 0, concurrent = 0, finalFailures = 0;
  const errors = [];
  const server = http.createServer(async (req, res) => {
    ++inFlight; concurrent = Math.max(concurrent, inFlight);
    res.on('close', () => --inFlight);
    try {
      assert.equal(req.method, 'POST');
      assert.equal(req.url, '/controller?station=engineer');
      assert.match(req.headers['content-type'], /^application\/json/);
      let body = ''; for await (const chunk of req) body += chunk;
      const value = JSON.parse(body); received.push({ at: Date.now(), value });
      assert.equal(Object.keys(value.powerPercentages).length, 8);
      assert.equal(Object.keys(value.coolantWeights).length, 8);
      assert.equal(Object.keys(value.coolant.percentages).length, 8);
      assert.equal(value.shipName, 'ESS Endeavour');
      if (fail) await new Promise(r => setTimeout(r, 120));
      const reject = fail && (received.length === 1 || (value.final && finalFailures++ === 0));
      res.writeHead(reject ? 503 : 204); res.end();
    } catch (error) { errors.push(error.message); res.writeHead(500); res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const reportPath = path.join(artifacts, `http-${name}.json`);
  try {
    const code = await run(['--network-test', `http://127.0.0.1:${server.address().port}/controller?station=engineer`, reportPath]);
    const report = JSON.parse(fs.readFileSync(reportPath));
    assert.equal(code, 0, JSON.stringify(report));
    assert.equal(report.passed, true);
    assert.equal(errors.length, 0, errors.join('\n'));
    assert.equal(concurrent, 1, 'Only one request in flight');
    assert.ok(received.length < 150, 'Intermediate updates must be coalesced');
    assert.ok(received.length > 1);
    const last = received.at(-1).value;
    assert.equal(last.final, true);
    assert.equal(last.sequence, report.finalSequence);
    assert.equal(last.powerPercentages.reactor, 150, 'Final snapped value');
    assert.equal(report.acknowledged, report.finalSequence);
    for (let i = 1; i < received.length; ++i) assert.ok(received[i].value.sequence >= received[i - 1].value.sequence, 'No stale request after a newer state');
    if (fail) {
      assert.ok(report.failed >= 2);
      assert.ok(received.filter(r => r.value.final && r.value.sequence === report.finalSequence).length >= 2, 'Failed final state is retried without further input');
    }
    fs.writeFileSync(path.join(artifacts, `http-${name}-receiver.json`), JSON.stringify({ passed: true, concurrent, report, received }, null, 2));
    return { name, requests: received.length, failures: report.failed, acknowledged: report.acknowledged };
  } finally {
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
  }
}
async function demoTest() {
  const reserve = http.createServer();
  await new Promise(resolve => reserve.listen(0, '127.0.0.1', resolve));
  const port = reserve.address().port; await new Promise(resolve => reserve.close(resolve));
  const child = spawn(process.execPath, [path.join(root, 'demo-receiver.cjs')], { env: { ...process.env, PORT: String(port) }, windowsHide: true });
  const closed = new Promise(resolve => child.on('exit', resolve));
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Demo receiver startup timeout')), 5000);
      child.stdout.on('data', chunk => { if (String(chunk).includes('Demo receiver:')) { clearTimeout(timer); resolve(); } });
      child.stderr.on('data', chunk => { clearTimeout(timer); reject(new Error(String(chunk))); });
      child.on('error', error => { clearTimeout(timer); reject(error); });
    });
    const report = path.join(artifacts, 'http-demo.json');
    assert.equal(await run(['--network-test', `http://127.0.0.1:${port}/controller`, report]), 0);
    const latest = JSON.parse(fs.readFileSync(path.join(artifacts, 'demo-latest-state.json')));
    assert.equal(latest.final, true); assert.equal(latest.powerPercentages.reactor, 150);
    return { passed: true, finalSequence: latest.sequence, reactor: latest.powerPercentages.reactor };
  } finally { child.kill(); await closed; }
}
async function diagnosticIsolationTest() {
  let requests = 0;
  const server = http.createServer((req, res) => { ++requests; res.writeHead(204); res.end(); });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const endpoint = `http://127.0.0.1:${server.address().port}/controller`;
    const transport = path.join(artifacts, 'enabled-transport.json');
    fs.writeFileSync(transport, JSON.stringify({ enabled: true, endpoint, sendHz: 30 }));
    assert.equal(await run(['--transport', transport, '--screenshot', path.join(artifacts, 'isolated-diagnostic.png')]), 0);
    assert.equal(requests, 0, 'Diagnostic render must disable configured HTTP delivery');
    return { passed: true, requests };
  } finally {
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
  }
}
(async () => {
  const uiPath = path.join(artifacts, 'ui-verification.json');
  assert.equal(await run(['--verify', uiPath]), 0);
  const ui = JSON.parse(fs.readFileSync(uiPath)); assert.equal(ui.passed, true);
  const rejected = [];
  for (const [name, args] of [
    ['missing-config', ['--config', path.join(artifacts, 'does-not-exist.json')]],
    ['bad-endpoint', ['--endpoint', 'file:///invalid']],
    ['empty-endpoint', ['--endpoint', '']],
    ['missing-transport', ['--transport', path.join(artifacts, 'does-not-exist-transport.json')]],
  ]) {
    const report = path.join(artifacts, `${name}.json`);
    assert.equal(await run(['--verify', report, ...args]), 1);
    assert.equal(typeof JSON.parse(fs.readFileSync(report)).error, 'string');
    rejected.push(name);
  }
  const network = [];
  network.push(await receiverTest('normal', false));
  network.push(await receiverTest('slow-retry', true));
  const demo = await demoTest();
  const diagnosticIsolation = await diagnosticIsolationTest();
  const result = { passed: true, ui, rejected, network, demo, diagnosticIsolation };
  fs.writeFileSync(path.join(artifacts, 'verification.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
})().catch(error => { console.error(error); process.exitCode = 1; });
