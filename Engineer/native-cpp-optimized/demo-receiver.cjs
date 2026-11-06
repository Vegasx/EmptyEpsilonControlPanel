// Development-only receiver. The C++ controller itself does not require Node.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const keys = ['reactor', 'beamweapons', 'missilesystem', 'maneuver', 'impulse', 'warp', 'frontshield', 'rearshield'];
const port = Number(process.env.PORT || 4181);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be 1..65535');
const output = path.join(__dirname, 'artifacts/demo-latest-state.json');
fs.mkdirSync(path.dirname(output), { recursive: true });
const server = http.createServer(async (req, res) => {
  if (req.method !== 'POST' || req.url !== '/controller') { res.writeHead(404); res.end(); return; }
  if (!/^application\/json(?:;|$)/i.test(req.headers['content-type'] || '')) { res.writeHead(415); res.end(); return; }
  try {
    const chunks = []; let bytes = 0;
    for await (const chunk of req) { bytes += chunk.length; if (bytes > 16384) { res.writeHead(413); res.end(); return; } chunks.push(chunk); }
    const state = JSON.parse(Buffer.concat(chunks).toString('utf8'));
    if (state.version !== 1 || typeof state.shipName !== 'string' || !Number.isSafeInteger(state.sequence) || typeof state.final !== 'boolean') throw new Error('Invalid state envelope');
    for (const bank of [state.powerPercentages, state.coolantWeights]) {
      if (!bank || Object.keys(bank).length !== keys.length || !keys.every(k => Number.isInteger(bank[k]) && bank[k] >= 0 && bank[k] <= 300)) throw new Error('Invalid slider values');
    }
    const total = keys.reduce((sum, k) => sum + state.coolantWeights[k], 0);
    if (!state.coolant || state.coolant.totalWeight !== total || !keys.every(k => Math.abs(state.coolant.percentages[k] - state.coolantWeights[k] / Math.max(300, total) * 100) < 1e-9)) throw new Error('Invalid coolant allocation');
    fs.writeFileSync(output, JSON.stringify({ receivedAt: new Date().toISOString(), ...state }, null, 2));
    console.log(`#${state.sequence}${state.final ? ' final' : ''} | power: ${keys.map(k => state.powerPercentages[k]).join(', ')} | coolant weights: ${keys.map(k => state.coolantWeights[k]).join(', ')}`);
    res.writeHead(204); res.end();
  } catch (error) { console.error(error.message); res.writeHead(400); res.end('Invalid controller state'); }
});
server.requestTimeout = 5000;
server.on('error', error => { console.error(error.message); process.exitCode = 1; });
server.listen(port, '127.0.0.1', () => {
  console.log(`Demo receiver: http://127.0.0.1:${port}/controller`);
  console.log(`Latest state: ${output}`);
  console.log('Launch: .\\"Start Cpp Controller.cmd" --endpoint http://127.0.0.1:' + port + '/controller');
  console.log('Ctrl+C stops this receiver. This is not connected to the game.');
});
