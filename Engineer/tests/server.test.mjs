import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import http from 'node:http';
import {fileURLToPath} from 'node:url';

const serverFile = fileURLToPath(new URL('../server.mjs', import.meta.url));
function start(port) {
  const child = spawn(process.execPath, [serverFile], {
    env:{...process.env, PORT:String(port), HOST:'127.0.0.1'}, windowsHide:true,
    stdio:['ignore','pipe','pipe']
  });
  let output = '';
  let stderr = '';
  child.stdout.on('data', data => { output += data; });
  child.stderr.on('data', data => { stderr += data; });
  const exited = once(child, 'exit');
  return {
    child, exited,
    get output() { return output; },
    get stderr() { return stderr; },
    async ready() {
      for (let count = 0; count < 100; count++) {
        const match = output.match(/Engineering controller: (http:\/\/[^\s]+)/);
        if (match) return match[1];
        if (child.exitCode !== null) throw new Error(stderr || output);
        await new Promise(resolve => setTimeout(resolve, 30));
      }
      throw new Error('Controller did not become ready');
    }
  };
}

test('launching twice reuses the running controller without a crash', {timeout:10000}, async t => {
  const first = start(0);
  t.after(() => first.child.kill());
  const url = await first.ready();
  assert.equal((await fetch(url)).status, 200);
  const second = start(new URL(url).port);
  t.after(() => second.child.kill());
  const [code] = await second.exited;
  assert.equal(code, 0);
  assert.match(second.output, /Controller already running:/);
  assert.equal(second.stderr, '');
  assert.equal((await fetch(url)).status, 200);
});

test('a different program keeps its address and controller finds a free one', {timeout:10000}, async t => {
  const other = http.createServer((req,res) => res.end('Another application'));
  other.listen(0, '127.0.0.1');
  await once(other, 'listening');
  t.after(() => other.close());
  const port = other.address().port;
  const controller = start(port);
  t.after(() => controller.child.kill());
  const url = await controller.ready();
  assert.notEqual(Number(new URL(url).port), port);
  assert.equal((await fetch(url)).status, 200);
  assert.equal(await (await fetch(`http://127.0.0.1:${port}`)).text(), 'Another application');
  assert.equal(controller.stderr, '');
});

test('invalid address setting produces a readable message', {timeout:10000}, async () => {
  const controller = start('invalid');
  const [code] = await controller.exited;
  assert.equal(code, 1);
  assert.match(controller.stderr, /Could not start the controller: PORT must be/);
  assert.doesNotMatch(controller.stderr, /Unhandled|throw er|node:events/);
});

test('outdated controller is not reused after a server update', {timeout:10000}, async t => {
  const current = start(0);
  t.after(() => current.child.kill());
  const currentUrl = await current.ready();
  const identity = await (await fetch(new URL('__controller/status', currentUrl))).json();
  assert.match(identity.serverRevision, /^[a-f0-9]{64}$/);
  const outdated = http.createServer((req,res) => {
    res.writeHead(200, {'Content-Type':'application/json'});
    res.end(JSON.stringify({...identity, serverRevision:'older-server'}));
  });
  outdated.listen(0, '127.0.0.1');
  await once(outdated, 'listening');
  t.after(() => outdated.close());
  const next = start(outdated.address().port);
  t.after(() => next.child.kill());
  const nextUrl = await next.ready();
  assert.notEqual(Number(new URL(nextUrl).port), outdated.address().port);
  const grip = await fetch(new URL('assets/slider-candidates/02-brass-grip.png', nextUrl));
  assert.equal(grip.status, 200);
  assert.match(grip.headers.get('content-type'), /image\/png/);
  const bytes = new Uint8Array(await grip.arrayBuffer());
  assert.deepEqual([...bytes.slice(0,8)], [137,80,78,71,13,10,26,10]);
});
