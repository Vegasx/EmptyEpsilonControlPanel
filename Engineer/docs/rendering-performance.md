# Rendering optimization — 2026-11-05

Slider handles now use translate3d transforms and fills use scaleX transforms with fixed layout dimensions. The browser runs the 10% overshoot and shrinking corrections as Web Animations keyframes; application JavaScript no longer writes left, width or inherited position variables every animation frame. Geometry is cached and refreshed through ResizeObserver. Interrupted motion is sampled from animation time before retargeting.

Readouts keep their text nodes and update only when the displayed number changes. Labels, mode attributes and ruler ticks also avoid redundant writes. Native range inputs still handle touch, keyboard and accessibility. Stored power/coolant settings, snapping, reset isolation and normalized coolant allocation remain unchanged. Reduced-motion mode skips animation.

## Local comparison

Installed Chrome, headless, 1920×1080 viewport; eight mode switches after startup and artwork loading, with the same 310ms spacing in each run:

| Metric | Before | After |
| --- | ---: | ---: |
| Layout passes | 50 | 12 |
| Style recalculations | 91 | 56 |
| Layout time | 56.03ms | 15.47ms |
| Style recalculation time | 391.91ms | 126.09ms |

This is one local comparison, not a frame-rate guarantee for another device. It shows a 76% reduction in layout passes. A separate measured interval during an animated reset had zero layout passes after the initial value update.

Browser interaction checks verified exact overshoot/correction positions, continuous reversal, touch and keyboard input, active-profile reset, viewport resizing and reduced motion. All 23 state/startup/allocation checks passed.

Background-color and text-color mode transitions can still paint their affected elements. The full-screen paper texture filter remains removed. See https://web.dev/articles/animations-guide for the distinction between transform animations and layout/paint work.
