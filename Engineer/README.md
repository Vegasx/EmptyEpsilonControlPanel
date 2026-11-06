# Epsilon Engineering

A standalone touch-friendly practice controller inspired by the physical engineering panel. Written in browser-native JavaScript, HTML and CSS, with a small Node.js static server and no installation dependencies. No game connection or simulated telemetry is included.

Windows desktop alternatives are available alongside the browser app: [C# / WPF](native-windows/README.md) and the [C++ / Direct2D comparison prototype](native-cpp/README.md). Launch the latter with `native-cpp/Start Cpp Controller.cmd`. It includes a configurable background JSON POST sender and a local demo receiver; game-specific server integration remains separate. See its [performance report](native-cpp/PERFORMANCE.md) for the measured comparison.

## Run

A separate [native Rust / Direct2D controller](native-rust/README.md) matches the optimized C++ controller's appearance, controls, animation rules and background HTTP contract. Launch `native-rust/Start Rust Controller.cmd`, or copy its portable distribution folder to a Windows x64 tablet. It runs as a native executable without a browser or runtime installation. Performance measurements are left to independent testing.

With Node.js 20 or newer installed, open a terminal in this folder and run:

```sh
npm start
```

Open the address printed in the window (normally http://localhost:4180) in a modern browser. The Windows launcher opens Chrome automatically in a separate full-screen controller window. Press F11 to leave fullscreen or Alt+F4 to close it. Opening the URL manually still supports F11. The panel fits the window without document scrolling. Stop the server with Ctrl+C. `Start Controller.cmd` starts the server and opens the browser on Windows after the server is ready. Keep the first controller window open while playing. Opening the launcher again reuses an up-to-date controller from this folder and opens a new full-screen window; if another program occupies the address, the controller tries the next free port. It never stops that other program.

For a tablet or friends on the same local network, run with `HOST=0.0.0.0` (PowerShell: `$env:HOST='0.0.0.0'; npm start`) and open `http://<this-computer's-LAN-IP>:4180` on the tablet, using the port printed in the controller window if it differs from 4180. Windows may ask to allow Node.js on your private network. Each browser is an independent practice controller; sessions do not synchronize.

## Controls and defaults

- Power sliders run from 0% to 300%. Coolant sliders keep 0–300 relative weights but display their actual percentage of the shared coolant pool. Both use one-point position steps. Touch or click the track, drag the hexagonal handle, or use keyboard arrows, Home and End.
- The bottom-right rocker switches the same eight sliders between independent power and coolant values. Switching mode preserves both sets.
- Reset Levels restores only the currently selected profile (power or coolant) from its JSON defaults. The other profile keeps all its current slider values.
- Edit `config.json` and reload the page to change defaults.
- No browser storage is used. Reloading the page starts at bundled defaults in power mode.

`config.json` has version `1`, a `shipName` (1–60 characters), and `defaults.power` / `defaults.coolant` objects. Each object must contain all eight system keys with whole-number values from 0 to 300. The included values are sample practice defaults; change them to suit your crew.

## Future Empty Epsilon connection

Empty Epsilon's official [HTTP Server documentation](https://github.com/daid/EmptyEpsilon/wiki/HTTP-Server) describes browser access through `get.lua`, `set.lua` and `exec.lua`, plus hosting HTML/JavaScript from its `www` directory. This makes a JavaScript controller a practical fit without requiring it to be written in the game's native language.

`model.mjs` owns independent requested levels. `adapter.mjs` contains a local-only adapter with `applyLevels({ power, coolant })`, called on level changes, reset and configuration replacement. This is the seam for a future HTTP transport. System keys use the engineering identifiers reactor, beamweapons, missilesystem, maneuver, impulse, warp, frontshield and rearshield; verify these against the installed game version before integration.

Power percentages map to a 0.0–3.0 multiplier by dividing by 100. Coolant shares come from getCoolantAllocation(weights) in model.mjs. The adapter still receives raw 0–300 weights; a live adapter must normalize them and map the shares to the selected ship's coolant capacity and budget. A real connection must also select the ship, distinguish requested vs. actual allocations, read authoritative state, handle other stations' changes, throttle outgoing updates and surface connection errors. Serve from the game origin or use a local proxy if cross-origin access is unavailable. Keep the game's experimental HTTP server on a trusted private network as described in its documentation.

## Verify

Run `npm test` for state/configuration tests. Browser interaction checks and physical tablet testing are still pending. Verify touch dragging, keyboard input, independent banks, custom JSON/reset, invalid imports and mobile overflow before crew testing. Live game integration is not implemented.

Panel typography uses the bundled Russo One font, distributed under FONT-LICENSE.txt. The simplified panel keeps system labels, percentages, reset and mode selection; the panel fills the viewport; the toolbar has been removed.

All eight slider thumbs use assets/slider-candidates/02-brass-grip.png in both power and coolant modes. Restart the local server after updating its asset list, then refresh the browser.

Slider snapping applies on pointer/touch release: within 1 point of any multiple of 5, or within 3 points of 100, 150, 200, 250 and 300. Major stops take priority. Other values stay exact. Keyboard arrows retain 1-point movement while focused and snap on blur. Reset/configuration values are applied exactly, without snapping.

The controller launcher uses a dedicated Chrome profile at %LOCALAPPDATA%\EpsilonController\ChromeProfiles\<project-id>. It does not alter normal Chrome profiles, shortcuts, default-browser settings, or existing windows. If Chrome is not installed, it opens the default browser with manual F11 fullscreen. Browser startup uses Chrome's --start-fullscreen and --user-data-dir flags, documented in https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md and Chromium's content/public/common/content_switches.cc.

Switching modes and Reset Levels animate with linear travel to 10% beyond the target, then four end corrections with errors of -1%, +0.1%, -0.01% of the original distance, and finally exactly zero. Shorter moves arrive sooner; a full-range move and corrections complete in 250ms. Overshoot is visual only, including at the ends of the track: stored settings and actual allocations never leave their valid ranges, and fill remains within the rail. Rail and fill colors blend linearly between red and blue over 250ms. Rapid switches and resets continue from the visible position; dragging takes control immediately. Reset affects only the active profile. Reduced-motion preference disables movement and color transitions.

Coolant uses a shared 100% allocation: share = slider weight / max(300, sum of all coolant weights) × 100. Below a combined full bar, spare coolant stays unallocated; above that threshold, requests are shared proportionally within the 100% pool. One full slider shows 100%, two show 50% each, and three show approximately 33% each. The ruler maximum is min(100, 300 / total weight × 100), so it reads 100%, 50% or 33% in those examples and never exceeds 100%. A lone slider one-third across now receives 33%, leaving 67% unused. Two sliders at 30% and 20% allocate 50% combined, leaving 50% unused. The ruler stays capped at 100%. With all coolant sliders off, shares are 0% and the empty ruler uses 0–100%. Displayed labels are rounded to whole percentages; calculations keep full precision. Power stays on its fixed 0–300% ruler. Reset remains specific to the active profile and its color follows that profile.
