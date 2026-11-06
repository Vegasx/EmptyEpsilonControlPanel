use crate::{model::*, now, panel::Panel, render, win, write};
use anyhow::{ensure, Result};
use windows::Win32::{
    Foundation::*,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
impl Panel {
    pub unsafe fn verify(&mut self) -> Result<()> {
        let mut checks = Vec::new();
        let mut check = |b: bool, name: &str| -> Result<()> {
            ensure!(b, "UI check failed: {name}");
            checks.push(name.to_owned());
            Ok(())
        };
        let original = self.state.clone();
        let reduced = self.reduced;
        self.reduced = true;
        self.focus = 0;
        self.state.cooling = false;
        self.state.power[0] = 100;
        self.update_motion(false);
        self.key(VK_RIGHT.0 as usize)?;
        check(self.state.power[0] == 101, "keyboard unsnapped step")?;
        self.key(VK_TAB.0 as usize)?;
        check(self.state.power[0] == 100, "focus loss snap")?;
        let bank = self.state.power;
        self.toggle();
        check(
            self.state.cooling && self.state.power == bank,
            "independent power profile",
        )?;
        self.state.coolant = [300; 8];
        let a = allocate(&self.state.coolant);
        check(
            a.maximum == 12.5 && a.percentages == [12.5; 8],
            "coolant pool and ruler",
        )?;
        self.state.set(0, 100.0);
        check(self.state.coolant[1] == 300, "independent coolant weights")?;
        self.reset();
        check(
            self.state.coolant == self.state.config.coolant && self.state.power == bank,
            "reset active profile only",
        )?;
        self.toggle();
        let y = self.g.body_top + self.g.row / 2.0;
        self.begin(self.g.slider_right - self.g.grip_size / 2.0, y, true, 0);
        check(self.state.power[0] == 300, "rail endpoint")?;
        self.update_drag(self.g.slider_left - 100.0);
        self.finish(false, -1.0, -1.0);
        check(
            self.state.power[0] == 0 && self.dragging == -1 && GetCapture() != self.hwnd,
            "capture and slider bounds",
        )?;
        self.begin(self.g.slider_left + self.g.grip_size / 2.0, y, false, 42);
        self.update_drag(self.g.slider_right);
        self.finish(false, -1.0, -1.0);
        check(
            self.state.power[0] == 300 && self.pointer == 0,
            "pointer release path",
        )?;
        self.fullscreen_toggle()?;
        check(self.fullscreen, "enter fullscreen")?;
        self.fullscreen_toggle()?;
        check(!self.fullscreen, "leave fullscreen")?;
        self.drain()?;
        self.state = original.clone();
        self.focus = 0;
        self.keyboard_editing = false;
        self.update_motion(false);
        send(
            self.hwnd,
            WM_KEYDOWN,
            WPARAM(VK_RIGHT.0 as usize),
            LPARAM(0),
        );
        self.drain()?;
        check(self.state.power[0] == 101, "real HWND keyboard dispatch")?;
        send(self.hwnd, WM_KILLFOCUS, WPARAM(0), LPARAM(0));
        self.drain()?;
        check(
            self.state.power[0] == 100 && !self.keyboard_editing,
            "real HWND focus-loss commit",
        )?;
        let pack = |x: f32, y: f32| {
            LPARAM(
                (((y * self.dpi / 96.0).round() as u16 as u32) << 16
                    | ((x * self.dpi / 96.0).round() as u16 as u32)) as isize,
            )
        };
        let y = self.g.body_top + self.g.row / 2.0;
        let end = pack(self.g.slider_right - self.g.grip_size / 2.0, y);
        let middle = pack(
            self.g.slider_left
                + self.g.grip_size / 2.0
                + (self.g.slider_right - self.g.slider_left - self.g.grip_size) * 147.0 / 300.0,
            y,
        );
        send(self.hwnd, WM_LBUTTONDOWN, WPARAM(1), end);
        self.drain()?;
        check(
            self.state.power[0] == 300 && GetCapture() == self.hwnd,
            "real HWND mouse capture",
        )?;
        send(self.hwnd, WM_MOUSEMOVE, WPARAM(1), middle);
        self.drain()?;
        send(self.hwnd, WM_LBUTTONUP, WPARAM(0), middle);
        self.drain()?;
        check(
            self.state.power[0] == 150 && self.dragging == -1 && GetCapture() != self.hwnd,
            "real HWND release snapping",
        )?;
        let power = self.state.power;
        self.focus = 9;
        send(
            self.hwnd,
            WM_KEYDOWN,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        );
        self.drain()?;
        check(
            self.state.cooling && self.state.power == power,
            "real HWND rocker keyboard activation",
        )?;
        self.state = original.clone();
        self.reduced = false;
        self.show_focus = false;
        self.focus = 0;
        self.keyboard_editing = false;
        self.color_since = now() - 1000.0;
        self.rocker_since = self.color_since;
        self.update_motion(false);
        self.state.set(0, 300.0);
        self.update_motion(false);
        self.reset();
        let since = self.motion[0].since;
        self.reset();
        check(
            self.motion[0].since == since && self.motion[0].active,
            "repeated reset preserves motion clock",
        )?;
        check(!self.motion[1].active, "unchanged sliders stay static")?;
        self.motion[0].since -= 1000.0;
        self.moving(now());
        self.paint(vec![self.g.physical(self.g.full(), self.dpi)])?;
        let seq = self.sequence;
        let draws = self.draws;
        self.reset();
        self.paint(Vec::new())?;
        check(
            self.draws == draws && self.sequence == seq + 1,
            "settled reset sends final state without redraw",
        )?;
        check(
            !self.clock.as_ref().unwrap().is_active(),
            "display clock sleeps at idle",
        )?;
        self.toggle();
        self.motion.iter_mut().for_each(|m| m.since -= 50.0);
        self.color_since -= 50.0;
        self.rocker_since -= 50.0;
        self.paint(Vec::new())?;
        check(self.partial_draws > 0, "retained partial window painting")?;
        check(
            self.last_area
                < self.g.width as f64 * self.g.height as f64 * (self.dpi as f64 / 96.0).powi(2),
            "bounded update area",
        )?;
        self.motion.iter_mut().for_each(|m| m.active = false);
        self.paint(Vec::new())?;
        let masks = self.graphics.mask_builds;
        let backgrounds = self.graphics.background_builds;
        self.color_since -= 40.0;
        self.rocker_since -= 40.0;
        self.paint(Vec::new())?;
        check(
            self.graphics.mask_builds == masks && self.graphics.background_builds == backgrounds,
            "color transition reuses background and masks",
        )?;
        self.graphics.discard();
        self.last = None;
        let full = self.full_draws;
        self.paint(Vec::new())?;
        check(
            self.full_draws == full + 1 && self.graphics.background_builds > backgrounds,
            "render-target replacement rebuilds caches",
        )?;
        self.state = original.clone();
        self.reduced = reduced;
        self.color_since = now() - 1000.0;
        self.rocker_since = self.color_since;
        self.update_motion(false);
        self.show_focus = false;
        let dir = self
            .options
            .verify
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let dir = dir.to_path_buf();
        self.screenshot(&dir.join("rust-panel.png"))?;
        for (w, h, name) in [(640, 480, "minimum"), (768, 1024, "portrait")] {
            self.resize_client(w, h)?;
            check(
                self.g.body_top + 8.0 * self.g.row <= self.g.reset.top
                    && self.g.grip_size <= self.g.row,
                "rows and footer fit on resize",
            )?;
            self.screenshot(&dir.join(format!("rust-panel-{name}.png")))?;
        }
        self.state.cooling = true;
        self.update_motion(false);
        self.color_since = now() - 1000.0;
        self.rocker_since = self.color_since;
        self.screenshot(&dir.join("rust-panel-coolant.png"))?;
        self.state = original;
        self.resize_client(1000, 700)?;
        self.update_motion(false);
        self.color_since = now() - 1000.0;
        self.rocker_since = self.color_since;
        let pixel_checks = self.verify_pixels()?;
        check(
            pixel_checks == 15,
            "partial versus full rendering at three DPI settings",
        )?;
        let (vblanks, fallbacks) = self.clock.as_ref().unwrap().counts();
        write(
            &self.options.verify,
            &serde_json::json!({"passed":true,"checks":checks.len(),"checkNames":checks,"pixelComparisons":pixel_checks,"physicalInputTested":false,"width":self.g.width,"height":self.g.height,"deviceScaleFactor":self.dpi/96.0,"partialDraws":self.partial_draws,"fullDraws":self.full_draws,"maskBuilds":self.graphics.mask_builds,"backgroundBuilds":self.graphics.background_builds,"vblankWaits":vblanks,"fallbackWaits":fallbacks}),
        )?;
        Ok(())
    }
    unsafe fn resize_client(&mut self, w: i32, h: i32) -> Result<()> {
        let mut r = RECT {
            left: 0,
            top: 0,
            right: (w as f32 * self.dpi / 96.0) as i32,
            bottom: (h as f32 * self.dpi / 96.0) as i32,
        };
        windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(
            &mut r,
            WINDOW_STYLE(GetWindowLongPtrW(self.hwnd, GWL_STYLE) as u32),
            false,
            WINDOW_EX_STYLE(0),
            self.dpi as u32,
        )?;
        SetWindowPos(
            self.hwnd,
            None,
            40,
            40,
            r.right - r.left,
            r.bottom - r.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
        self.resize()?;
        Ok(())
    }
    unsafe fn verify_pixels(&mut self) -> Result<usize> {
        let mut count = 0;
        let saved_g = self.g;
        let saved_dpi = self.dpi;
        for dpi in [96.0, 144.0, 192.0] {
            let g = render::Geometry::new(640.0, 480.0);
            let mut old = self.visual(now());
            old.positions = [0.0, 300.0, 100.0, 150.0, 0.0, 300.0, 100.0, 0.0];
            let mut cases = Vec::new();
            let mut v = old.clone();
            v.positions = [330.0, -30.0, 110.0, 149.0, 25.0, 275.0, 0.0, 300.0];
            cases.push(v);
            let mut v = old.clone();
            v.accent = render::color(0x287bb9, 1.0);
            v.rail = render::color(0x203f64, 1.0);
            v.cooling = true;
            v.lever = 0.7;
            cases.push(v);
            let mut v = old.clone();
            v.readouts[0] = "150%".into();
            v.ruler = ["0%".into(), "4%".into(), "8%".into(), "13%".into()];
            v.status = "RETRY: HTTP 503".into();
            cases.push(v);
            let mut v = old.clone();
            v.focus = 0;
            cases.push(v);
            let mut v = old.clone();
            v.focus = 9;
            cases.push(v);
            for v in cases {
                let (partial, rt, grip) = self.graphics.offscreen(g, dpi)?;
                self.graphics.clear();
                self.graphics.backdrop(&rt, g, dpi)?;
                rt.BeginDraw();
                self.graphics.region(&rt, &grip, g, dpi, g.full(), &old)?;
                rt.EndDraw(None, None)?;
                let physical: Vec<_> = render::changed(&g, Some(&old), &v, false)
                    .iter()
                    .map(|r| g.physical(*r, dpi))
                    .collect();
                rt.BeginDraw();
                for r in win::union_rectangles(&physical) {
                    self.graphics.region(
                        &rt,
                        &grip,
                        g,
                        dpi,
                        render::rect(
                            r.left as f32 * 96.0 / dpi,
                            r.top as f32 * 96.0 / dpi,
                            r.right as f32 * 96.0 / dpi,
                            r.bottom as f32 * 96.0 / dpi,
                        ),
                        &v,
                    )?;
                }
                rt.EndDraw(None, None)?;
                let (full, reference, grip) = self.graphics.offscreen(g, dpi)?;
                self.graphics.clear();
                self.graphics.backdrop(&reference, g, dpi)?;
                reference.BeginDraw();
                self.graphics
                    .region(&reference, &grip, g, dpi, g.full(), &v)?;
                reference.EndDraw(None, None)?;
                let width = (g.width * dpi / 96.0) as u32;
                let height = (g.height * dpi / 96.0) as u32;
                let mut a = vec![0u8; (width * height * 4) as usize];
                let mut b = a.clone();
                partial.CopyPixels(std::ptr::null(), width * 4, &mut a)?;
                full.CopyPixels(std::ptr::null(), width * 4, &mut b)?;
                let max = a
                    .iter()
                    .zip(&b)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap_or(0);
                ensure!(max<=2,"Partial render mismatch at {dpi} DPI case {count}: max channel difference {max}");
                count += 1;
            }
        }
        self.g = saved_g;
        self.dpi = saved_dpi;
        self.graphics.clear();
        self.last = None;
        Ok(count)
    }
}

unsafe fn send(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) {
    SendMessageW(hwnd, msg, Some(wp), Some(lp));
}
