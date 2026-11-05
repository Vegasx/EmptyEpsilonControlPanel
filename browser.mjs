import {existsSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {spawn} from 'node:child_process';

export function chromeLaunch(url, profileKey, {platform = process.platform, env = process.env, exists = existsSync} = {}) {
  if (platform !== 'win32') return null;
  const local = env.LOCALAPPDATA || path.win32.join(os.homedir(), 'AppData', 'Local');
  const candidates = [
    path.win32.join(env.ProgramFiles || 'C:\\Program Files', 'Google', 'Chrome', 'Application', 'chrome.exe'),
    path.win32.join(env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)', 'Google', 'Chrome', 'Application', 'chrome.exe'),
    path.win32.join(local, 'Google', 'Chrome', 'Application', 'chrome.exe')
  ];
  const command = candidates.find(exists);
  if (!command) return null;
  // A controller-specific user-data-dir keeps startup flags and preferences away
  // from normal Chrome windows. Never use or modify Chrome's default profile.
  const profile = path.win32.join(local, 'EpsilonController', 'ChromeProfiles', profileKey.slice(0,16));
  return {command, profile, args:[
    `--user-data-dir=${profile}`,
    `--app=${url}`,
    '--start-fullscreen',
    '--new-window',
    '--no-first-run',
    '--no-default-browser-check'
  ]};
}

function defaultBrowser(url) {
  const command = process.platform === 'win32' ? 'rundll32.exe' : process.platform === 'darwin' ? 'open' : 'xdg-open';
  const args = process.platform === 'win32' ? ['url.dll,FileProtocolHandler', url] : [url];
  const browser = spawn(command, args, {windowsHide:true, detached:true, stdio:'ignore'});
  browser.on('error', () => console.log(`Open this address in your browser: ${url}`));
  browser.unref();
}

export function openControllerBrowser(url, profileKey) {
  const launch = chromeLaunch(url, profileKey);
  if (!launch) {
    console.log('Opening your default browser. Press F11 for fullscreen.');
    defaultBrowser(url);
    return;
  }
  const browser = spawn(launch.command, launch.args, {windowsHide:false, detached:true, stdio:'ignore'});
  browser.on('error', () => {
    console.log('Could not open Chrome; opening your default browser.');
    defaultBrowser(url);
  });
  browser.unref();
}
