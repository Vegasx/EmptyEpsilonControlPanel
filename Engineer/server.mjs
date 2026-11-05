import http from 'node:http';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {openControllerBrowser} from './browser.mjs';

const root = fileURLToPath(new URL('.', import.meta.url));
const identity = {
  app: 'epsilon-engineering-controller',
  protocol: 1,
  serverRevision: createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).update(await readFile(new URL('./browser.mjs', import.meta.url))).digest('hex'),
  folderId: createHash('sha256').update(path.resolve(root).toLowerCase()).digest('hex')
};
const files = new Set(['index.html', 'style.css', 'app.mjs', 'model.mjs', 'adapter.mjs', 'config.json', 'RussoOne-Regular.ttf', 'FONT-LICENSE.txt', 'assets/slider-candidates/02-brass-grip.png']);
const types = {'.html':'text/html', '.css':'text/css', '.mjs':'text/javascript', '.json':'application/json', '.ttf':'font/ttf', '.txt':'text/plain', '.png':'image/png'};

function createServer() {
  return http.createServer(async (req, res) => {
    try {
      const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
      if (pathname === '/__controller/status') {
        res.writeHead(200, {'Content-Type':'application/json', 'Cache-Control':'no-store'});
        res.end(req.method === 'HEAD' ? undefined : JSON.stringify(identity));
        return;
      }
      const file = pathname === '/' ? 'index.html' : pathname.slice(1);
      if (!files.has(file)) { res.writeHead(404).end('Not found'); return; }
      const data = await readFile(path.join(root, file));
      res.writeHead(200, {'Content-Type':`${types[path.extname(file)]}; charset=utf-8`, 'Cache-Control':'no-store'});
      res.end(req.method === 'HEAD' ? undefined : data);
    } catch { res.writeHead(500).end('Unable to read controller file'); }
  });
}

function listen(server, port, host) {
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, () => {
      server.removeListener('error', reject);
      resolve();
    });
  });
}

function browserUrl(host, port) {
  const address = ['0.0.0.0', '::'].includes(host) ? '127.0.0.1' : host;
  return `http://${address.includes(':') ? `[${address}]` : address}:${port}/`;
}

async function isThisController(url) {
  try {
    const response = await fetch(new URL('__controller/status', url), {signal:AbortSignal.timeout(1500), redirect:'error'});
    if (!response.ok) return false;
    const status = await response.json();
    return status.app === identity.app && status.protocol === identity.protocol && status.folderId === identity.folderId && status.serverRevision === identity.serverRevision;
  } catch { return false; }
}

function openBrowser(url) { openControllerBrowser(url, identity.folderId); }

async function main() {
  const requestedPort = Number(process.env.PORT || 4180);
  const host = process.env.HOST || '127.0.0.1';
  if (!Number.isInteger(requestedPort) || requestedPort < 0 || requestedPort > 65535) {
    throw new Error('PORT must be a whole number between 0 and 65535.');
  }
  for (let attempt = 0; attempt < 20 && requestedPort + attempt <= 65535; attempt++) {
    const port = requestedPort + attempt;
    const server = createServer();
    try {
      await listen(server, port, host);
    } catch (error) {
      if (error.code !== 'EADDRINUSE') throw error;
      const url = browserUrl(host, port);
      if (await isThisController(url)) {
        console.log(`Controller already running: ${url}`);
        if (process.argv.includes('--open')) openBrowser(url);
        return;
      }
      console.log(`Address ${port} is busy; trying another.`);
      continue;
    }
    const url = browserUrl(host, server.address().port);
    console.log(`Engineering controller: ${url}`);
    console.log('Keep this window open while using the controller.');
    if (process.argv.includes('--open')) openBrowser(url);
    return;
  }
  throw new Error('No free address found. Close an earlier controller window and try again.');
}

main().catch(error => {
  console.error(`Could not start the controller: ${error.message}`);
  process.exitCode = 1;
});
