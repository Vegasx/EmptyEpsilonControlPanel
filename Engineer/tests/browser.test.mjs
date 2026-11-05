import test from 'node:test';
import assert from 'node:assert/strict';
import {chromeLaunch} from '../browser.mjs';

const env = {ProgramFiles:'C:\\Program Files', 'ProgramFiles(x86)':'C:\\Program Files (x86)', LOCALAPPDATA:'C:\\Users\\Crew\\AppData\\Local'};
test('controller Chrome launch starts fullscreen in a dedicated profile', () => {
  const url='http://127.0.0.1:4182/';
  const launch=chromeLaunch(url,'1234567890abcdef1234567890abcdef',{platform:'win32',env,exists:()=>true});
  assert.equal(launch.command,'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe');
  assert.equal(launch.profile,'C:\\Users\\Crew\\AppData\\Local\\EpsilonController\\ChromeProfiles\\1234567890abcdef');
  assert.ok(launch.args.includes(`--app=${url}`));
  assert.ok(launch.args.includes('--start-fullscreen'));
  assert.ok(launch.args.includes(`--user-data-dir=${launch.profile}`));
  assert.ok(!launch.profile.includes('Google\\Chrome\\User Data'));
  assert.ok(!launch.args.includes('--kiosk'));
});
test('finds per-user Chrome installations', () => {
  const launch=chromeLaunch('http://localhost:4180/','profile',{platform:'win32',env,exists:p=>p.startsWith(env.LOCALAPPDATA)});
  assert.equal(launch.command,'C:\\Users\\Crew\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe');
});
test('missing Chrome and other platforms use the default-browser fallback', () => {
  assert.equal(chromeLaunch('http://localhost:4180/','profile',{platform:'win32',env,exists:()=>false}),null);
  assert.equal(chromeLaunch('http://localhost:4180/','profile',{platform:'darwin',env}),null);
});
