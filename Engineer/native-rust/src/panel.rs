use crate::{
    frame_clock::FrameClock,
    model::*,
    now, profile,
    render::{self, color, inside, mix, rect, Color, Geometry, Graphics, Visual},
    transport::{wide, Sender},
    win::{self, Event},
    Options,
};
use anyhow::{bail, ensure, Result};
use std::path::Path;
use windows::{
    core::{w, BOOL, PCWSTR},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{
            HiDpi::*,
            Input::{KeyboardAndMouse::*, Pointer::*},
            WindowsAndMessaging::*,
        },
    },
};
pub(crate) struct Panel {
    pub state: State,
    pub options: Options,
    pub hwnd: HWND,
    pub ctx: Box<win::Context>,
    pub g: Geometry,
    pub dpi: f32,
    pub graphics: Graphics,
    pub motion: [Motion; 8],
    pub sender: Option<Sender>,
    pub clock: Option<FrameClock>,
    pub sequence: u64,
    pub fullscreen: bool,
    saved_rect: RECT,
    pub reduced: bool,
    pub focus: i32,
    pub dragging: i32,
    pressed: i32,
    pub pointer: u32,
    mouse_drag: bool,
    pub keyboard_editing: bool,
    pub show_focus: bool,
    offset: f32,
    pub color_since: f64,
    pub rocker_since: f64,
    rocker_start: f64,
    accent_start: Color,
    rail_start: Color,
    pub last: Option<Visual>,
    pub draws: u64,
    pub full_draws: u64,
    pub partial_draws: u64,
    pub last_area: f64,
    pub profile: Option<profile::Profile>,
    pub destroyed: bool,
}
impl Panel {
    pub unsafe fn new(config: Configuration, options: Options) -> Result<Self> {
        let state = State::new(config);
        let motion = std::array::from_fn(|i| Motion {
            target: state.bank()[i] as f64,
            ..Default::default()
        });
        let mut p = Self {
            state,
            options,
            hwnd: HWND::default(),
            ctx: Box::new(win::Context::new()),
            g: Geometry::new(1000.0, 700.0),
            dpi: 96.0,
            graphics: Graphics::new()?,
            motion,
            sender: None,
            clock: None,
            sequence: 0,
            fullscreen: false,
            saved_rect: RECT::default(),
            reduced: false,
            focus: 0,
            dragging: -1,
            pressed: -1,
            pointer: 0,
            mouse_drag: false,
            keyboard_editing: false,
            show_focus: false,
            offset: 0.0,
            color_since: -1000.0,
            rocker_since: -1000.0,
            rocker_start: 0.0,
            accent_start: color(0xae3c32, 1.0),
            rail_start: color(0x592923, 1.0),
            last: None,
            draws: 0,
            full_draws: 0,
            partial_draws: 0,
            last_area: 0.0,
            profile: None,
            destroyed: false,
        };
        p.preference();
        Ok(p)
    }
    fn accent(&self, t: f64) -> Color {
        mix(
            self.accent_start,
            color(
                if self.state.cooling {
                    0x287bb9
                } else {
                    0xae3c32
                },
                1.0,
            ),
            (t - self.color_since) / 250.0,
        )
    }
    fn rail(&self, t: f64) -> Color {
        mix(
            self.rail_start,
            color(
                if self.state.cooling {
                    0x203f64
                } else {
                    0x592923
                },
                1.0,
            ),
            (t - self.color_since) / 250.0,
        )
    }
    fn rocker(&self, t: f64) -> f64 {
        self.rocker_start
            + ((self.state.cooling as i32 as f64) - self.rocker_start)
                * ((t - self.rocker_since) / 160.0).clamp(0.0, 1.0)
    }
    pub fn visual(&self, t: f64) -> Visual {
        let a = allocate(&self.state.coolant);
        Visual {
            positions: self.motion.map(|m| m.sample(t)),
            readouts: std::array::from_fn(|i| {
                format!(
                    "{}%",
                    (if self.state.cooling {
                        a.percentages[i]
                    } else {
                        self.state.power[i] as f64
                    })
                    .round() as i32
                )
            }),
            ruler: std::array::from_fn(|i| {
                format!(
                    "{}%",
                    ((if self.state.cooling { a.maximum } else { 300.0 }) * i as f64 / 3.0).round()
                        as i32
                )
            }),
            accent: self.accent(t),
            rail: self.rail(t),
            cooling: self.state.cooling,
            lever: self.rocker(t),
            focus: if self.show_focus { self.focus } else { -1 },
            status: if self.options.profile.as_os_str().is_empty() {
                self.sender.as_ref().map(|s| s.status()).unwrap_or_default()
            } else {
                String::new()
            },
        }
    }
    pub fn moving(&mut self, t: f64) -> bool {
        let mut active = false;
        for m in &mut self.motion {
            if m.active && t - m.since >= m.duration() {
                m.active = false;
            }
            active |= m.active;
        }
        active || t - self.color_since < 250.0 || t - self.rocker_since < 160.0
    }
    pub fn request_frame(&mut self) {
        if self.hwnd.is_invalid() || self.destroyed {
            return;
        }
        let t = now();
        let moving = self.moving(t);
        if let Some(c) = &self.clock {
            unsafe {
                c.active(
                    !IsIconic(self.hwnd).as_bool()
                        && (moving
                            || !render::changed(
                                &self.g,
                                self.last.as_ref(),
                                &self.visual(t),
                                self.graphics.full,
                            )
                            .is_empty()),
                );
            }
        } else {
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
    }
    pub fn update_motion(&mut self, animate: bool) {
        let t = now();
        for i in 0..8 {
            let value = self.state.bank()[i] as f64;
            let m = &mut self.motion[i];
            if !animate || self.reduced {
                if m.active || m.target != value {
                    m.retarget(value, t, false);
                }
            } else if m.target != value {
                m.retarget(value, t, true);
            }
        }
        self.request_frame();
    }
    fn send(&mut self, final_state: bool) {
        self.sequence += 1;
        if let Some(s) = &self.sender {
            s.submit(
                self.state.payload(self.sequence, final_state),
                self.sequence,
                final_state,
            );
        }
    }
    pub fn change_focus(&mut self, next: i32) {
        if self.keyboard_editing && self.focus < 8 {
            let i = self.focus as usize;
            self.state.commit(i);
            self.motion[i].retarget(self.state.bank()[i] as f64, now(), false);
            self.send(true);
        }
        self.keyboard_editing = false;
        self.focus = next;
        self.request_frame();
    }
    fn hit(&self, x: f32, y: f32) -> i32 {
        let g = self.g;
        if inside(g.reset, x, y) {
            8
        } else if inside(g.toggle, x, y) {
            9
        } else if x >= g.slider_left
            && x <= g.slider_right
            && y >= g.body_top
            && y < g.body_top + 8.0 * g.row
        {
            ((y - g.body_top) / g.row) as i32
        } else {
            -1
        }
    }
    pub fn update_drag(&mut self, x: f32) {
        if self.dragging < 0 {
            return;
        }
        let travel = self.g.slider_right - self.g.slider_left - self.g.grip_size;
        let i = self.dragging as usize;
        if travel > 0.0
            && self.state.set(
                i,
                ((x - self.g.slider_left - self.g.grip_size / 2.0 - self.offset) / travel * 300.0)
                    as f64,
            )
        {
            self.motion[i].retarget(self.state.bank()[i] as f64, now(), false);
            self.send(false);
            self.request_frame();
        }
    }
    pub unsafe fn begin(&mut self, x: f32, y: f32, mouse: bool, pointer: u32) {
        if self.dragging >= 0 || self.pressed >= 0 {
            return;
        }
        let control = self.hit(x, y);
        if control < 0 {
            return;
        }
        let _ = SetFocus(Some(self.hwnd));
        self.change_focus(control);
        self.show_focus = false;
        self.mouse_drag = mouse;
        self.pointer = pointer;
        if mouse {
            SetCapture(self.hwnd);
        }
        if control >= 8 {
            self.pressed = control;
        } else {
            self.dragging = control;
            let visible = self.motion[control as usize].sample(now());
            let center = self.g.slider_left
                + self.g.grip_size / 2.0
                + (visible / 300.0) as f32
                    * (self.g.slider_right - self.g.slider_left - self.g.grip_size);
            self.offset = if (x - center).abs() <= self.g.grip_size / 2.0 {
                x - center
            } else {
                0.0
            };
            self.motion[control as usize].retarget(visible, now(), false);
            self.update_drag(x);
        }
        self.request_frame();
    }
    pub unsafe fn finish(&mut self, activate: bool, x: f32, y: f32) {
        let button = self.pressed;
        self.pressed = -1;
        if self.dragging >= 0 {
            let i = self.dragging as usize;
            self.dragging = -1;
            self.state.commit(i);
            self.motion[i].retarget(self.state.bank()[i] as f64, now(), false);
            self.send(true);
        }
        if GetCapture() == self.hwnd {
            let _ = ReleaseCapture();
        }
        self.pointer = 0;
        self.mouse_drag = false;
        if activate && button >= 8 && self.hit(x, y) == button {
            if button == 8 {
                self.reset();
            } else {
                self.toggle();
            }
        }
        self.request_frame();
    }
    unsafe fn preference(&mut self) {
        let mut enabled = BOOL(1);
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        self.reduced = self.options.reduced || !enabled.as_bool();
        if self.reduced {
            self.color_since = now() - 1000.0;
            self.rocker_since = self.color_since;
            for m in &mut self.motion {
                m.active = false;
            }
        }
    }
    pub unsafe fn toggle(&mut self) {
        self.finish(false, -1.0, -1.0);
        self.change_focus(self.focus);
        let t = now();
        self.accent_start = self.accent(t);
        self.rail_start = self.rail(t);
        self.rocker_start = self.rocker(t);
        self.state.cooling = !self.state.cooling;
        self.color_since = if self.reduced { t - 1000.0 } else { t };
        self.rocker_since = self.color_since;
        self.update_motion(true);
    }
    pub unsafe fn reset(&mut self) {
        self.finish(false, -1.0, -1.0);
        self.change_focus(self.focus);
        self.state.reset();
        self.update_motion(true);
        self.send(true);
    }
    pub unsafe fn fullscreen_toggle(&mut self) -> Result<()> {
        let r = if !self.fullscreen {
            GetWindowRect(self.hwnd, &mut self.saved_rect)?;
            SetWindowLongPtrW(self.hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            ensure!(
                GetMonitorInfoW(
                    MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST),
                    &mut info
                )
                .as_bool(),
                "Cannot find monitor"
            );
            info.rcMonitor
        } else {
            SetWindowLongPtrW(
                self.hwnd,
                GWL_STYLE,
                (WS_OVERLAPPEDWINDOW | WS_VISIBLE).0 as isize,
            );
            self.saved_rect
        };
        SetWindowPos(
            self.hwnd,
            None,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            SWP_NOZORDER | SWP_FRAMECHANGED,
        )?;
        self.fullscreen = !self.fullscreen;
        self.resize()?;
        Ok(())
    }
    pub unsafe fn resize(&mut self) -> Result<()> {
        let mut r = RECT::default();
        GetClientRect(self.hwnd, &mut r)?;
        self.g = Geometry::new(
            r.right as f32 * 96.0 / self.dpi,
            r.bottom as f32 * 96.0 / self.dpi,
        );
        self.graphics
            .resize(r.right.max(0) as u32, r.bottom.max(0) as u32);
        self.last = None;
        self.request_frame();
        Ok(())
    }
    pub unsafe fn key(&mut self, key: usize) -> Result<()> {
        if key == VK_F11.0 as usize {
            return self.fullscreen_toggle();
        }
        if key == VK_TAB.0 as usize {
            self.finish(false, -1.0, -1.0);
            self.show_focus = true;
            self.change_focus(
                (self.focus
                    + if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        9
                    } else {
                        1
                    })
                    % 10,
            );
            return Ok(());
        }
        if self.focus >= 8 {
            if key == VK_SPACE.0 as usize || key == VK_RETURN.0 as usize {
                if self.focus == 8 {
                    self.reset();
                } else {
                    self.toggle();
                }
            }
            return Ok(());
        }
        let mut value = self.state.bank()[self.focus as usize];
        match key as u16 {
            k if k == VK_LEFT.0 || k == VK_DOWN.0 => value -= 1,
            k if k == VK_RIGHT.0 || k == VK_UP.0 => value += 1,
            k if k == VK_PRIOR.0 => value += 10,
            k if k == VK_NEXT.0 => value -= 10,
            k if k == VK_HOME.0 => value = 0,
            k if k == VK_END.0 => value = 300,
            _ => return Ok(()),
        }
        self.keyboard_editing = true;
        self.show_focus = true;
        let i = self.focus as usize;
        self.state.set(i, value as f64);
        self.motion[i].retarget(self.state.bank()[i] as f64, now(), false);
        self.send(false);
        self.request_frame();
        Ok(())
    }
    pub unsafe fn paint(&mut self, exposed: Vec<RECT>) -> Result<()> {
        if IsIconic(self.hwnd).as_bool() {
            return Ok(());
        }
        self.graphics.device(self.hwnd, self.dpi)?;
        if self.graphics.target.is_none() {
            return Ok(());
        }
        let (rt, grip) = self.graphics.window_resources();
        self.graphics.backdrop(&rt, self.g, self.dpi)?;
        let t = now();
        let v = self.visual(t);
        let full = self.graphics.full || self.last.is_none();
        let dirty = render::changed(&self.g, self.last.as_ref(), &v, full);
        let mut regions: Vec<RECT> = dirty
            .iter()
            .map(|r| self.g.physical(*r, self.dpi))
            .collect();
        regions.extend(exposed);
        let regions = win::union_rectangles(&regions);
        if regions.is_empty() {
            self.request_frame();
            return Ok(());
        }
        rt.BeginDraw();
        self.last_area = 0.0;
        let result = (|| -> Result<()> {
            for r in regions {
                self.last_area += ((r.right - r.left) * (r.bottom - r.top)) as f64;
                self.graphics.region(
                    &rt,
                    &grip,
                    self.g,
                    self.dpi,
                    rect(
                        r.left as f32 * 96.0 / self.dpi,
                        r.top as f32 * 96.0 / self.dpi,
                        r.right as f32 * 96.0 / self.dpi,
                        r.bottom as f32 * 96.0 / self.dpi,
                    ),
                    &v,
                )?;
            }
            Ok(())
        })();
        let end = rt.EndDraw(None, None);
        self.draws += 1;
        if full {
            self.full_draws += 1;
        } else {
            self.partial_draws += 1;
        }
        if let Some(p) = &mut self.profile {
            p.record_draw(t);
        }
        if end
            .as_ref()
            .is_err_and(|e| e.code() == D2DERR_RECREATE_TARGET)
        {
            self.graphics.discard();
            self.last = None;
            self.request_frame();
            return Ok(());
        }
        result?;
        end?;
        self.last = Some(v);
        self.graphics.full = false;
        self.request_frame();
        Ok(())
    }
    unsafe fn frame_ready(&mut self) {
        if let Some(c) = &self.clock {
            c.consumed();
            if IsIconic(self.hwnd).as_bool() {
                c.active(false);
                return;
            }
        }
        let dirty = render::changed(
            &self.g,
            self.last.as_ref(),
            &self.visual(now()),
            self.graphics.full,
        );
        for r in &dirty {
            let r = self.g.physical(*r, self.dpi);
            let _ = InvalidateRect(Some(self.hwnd), Some(&r), false);
        }
        if dirty.is_empty() {
            let moving = self.moving(now());
            if let Some(c) = &self.clock {
                c.active(moving);
            }
        }
    }
    pub unsafe fn screenshot(&mut self, path: &Path) -> Result<()> {
        let (bitmap, rt, grip) = self.graphics.offscreen(self.g, self.dpi)?;
        self.graphics.clear();
        self.graphics.backdrop(&rt, self.g, self.dpi)?;
        rt.BeginDraw();
        let result = self.graphics.region(
            &rt,
            &grip,
            self.g,
            self.dpi,
            self.g.full(),
            &self.visual(now()),
        );
        let end = rt.EndDraw(None, None);
        result?;
        end?;
        self.graphics.save_png(path, &bitmap)?;
        self.graphics.clear();
        self.last = None;
        Ok(())
    }
    pub unsafe fn handle(&mut self, e: Event) -> Result<()> {
        if self.destroyed {
            return Ok(());
        }
        match e {
            Event::Paint(exposed) => self.paint(exposed)?,
            Event::Resize(minimized) => {
                self.resize()?;
                if minimized {
                    if let Some(c) = &self.clock {
                        c.active(false);
                    }
                }
            }
            Event::Move => {
                if let Some(c) = &self.clock {
                    c.select_monitor(self.hwnd, false);
                }
            }
            Event::Display => {
                if let Some(c) = &self.clock {
                    c.select_monitor(self.hwnd, true);
                }
                self.graphics.clear();
                self.last = None;
                self.request_frame();
            }
            Event::Dpi(dpi, r) => {
                self.dpi = dpi;
                if let Some(t) = &self.graphics.target {
                    t.SetDpi(dpi, dpi);
                }
                SetWindowPos(
                    self.hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
                self.resize()?;
                if let Some(c) = &self.clock {
                    c.select_monitor(self.hwnd, false);
                }
            }
            Event::Timer(2) => self.profile_tick()?,
            Event::Timer(3) => {
                let _ = KillTimer(Some(self.hwnd), 3);
                if !self.options.verify.as_os_str().is_empty() {
                    self.verify()?;
                } else {
                    self.screenshot(&self.options.screenshot.clone())?;
                }
                DestroyWindow(self.hwnd)?;
            }
            Event::Timer(_) => {}
            Event::Mouse(WM_LBUTTONDOWN, x, y) => self.begin(x, y, true, 0),
            Event::Mouse(WM_MOUSEMOVE, x, _) => {
                if self.mouse_drag && self.dragging >= 0 {
                    self.update_drag(x);
                }
            }
            Event::Mouse(WM_LBUTTONUP, x, y) => {
                if self.mouse_drag {
                    self.update_drag(x);
                    self.finish(true, x, y);
                }
            }
            Event::Mouse(_, _, _) => {}
            Event::Pointer(msg, id, flags, x, y) => {
                if msg == WM_POINTERDOWN {
                    self.begin(x, y, false, id);
                } else if self.pointer == id {
                    self.update_drag(x);
                    if msg == WM_POINTERUP || flags & POINTER_FLAG_CANCELED.0 != 0 {
                        self.finish(
                            msg == WM_POINTERUP && flags & POINTER_FLAG_CANCELED.0 == 0,
                            x,
                            y,
                        );
                    }
                }
            }
            Event::Capture(id) => {
                if (id == 0 && self.mouse_drag) || (id != 0 && id == self.pointer) {
                    self.finish(false, -1.0, -1.0);
                }
            }
            Event::Cancel => self.finish(false, -1.0, -1.0),
            Event::Key(k) => self.key(k)?,
            Event::Blur => {
                self.finish(false, -1.0, -1.0);
                self.change_focus(self.focus);
                self.show_focus = false;
                self.request_frame();
            }
            Event::Preference => {
                self.preference();
                self.request_frame();
            }
            Event::Network => self.request_frame(),
            Event::Frame => self.frame_ready(),
            Event::Destroy => {
                self.destroyed = true;
                if let Some(c) = &mut self.clock {
                    c.stop();
                }
                if let Some(s) = &mut self.sender {
                    s.stop();
                }
                PostQuitMessage(0);
            }
        }
        Ok(())
    }
    pub unsafe fn drain(&mut self) -> Result<()> {
        loop {
            let e = self.ctx.queue.borrow_mut().pop_front();
            match e {
                Some(e) => self.handle(e)?,
                None => break,
            }
        }
        Ok(())
    }
    unsafe fn create_window(&mut self) -> Result<()> {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let cls = WNDCLASSW {
            lpfnWndProc: Some(win::procedure),
            hInstance: instance,
            lpszClassName: w!("EpsilonRustPanel"),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        ensure!(RegisterClassW(&cls) != 0, "Cannot register window");
        let diagnostic = self.options.diagnostic();
        let dpi = GetDpiForSystem();
        self.dpi = dpi as f32;
        self.ctx.dpi.set(self.dpi);
        let mut r = RECT {
            left: 0,
            top: 0,
            right: (if diagnostic { 1000 } else { 1280 }) * dpi as i32 / 96,
            bottom: (if diagnostic { 700 } else { 800 }) * dpi as i32 / 96,
        };
        let style = if !self.options.profile.as_os_str().is_empty() {
            WS_POPUP
        } else {
            WS_OVERLAPPEDWINDOW
        };
        AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE(0), dpi)?;
        let title = wide(&format!(
            "Epsilon Engineering Rust · {}",
            self.state.config.ship
        ));
        self.hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            cls.lpszClassName,
            PCWSTR(title.as_ptr()),
            style,
            40,
            40,
            r.right - r.left,
            r.bottom - r.top,
            None,
            None,
            Some(instance),
            Some(&*self.ctx as *const _ as _),
        )?;
        self.dpi = GetDpiForWindow(self.hwnd) as f32;
        self.ctx.dpi.set(self.dpi);
        self.sender = Some(Sender::new(self.options.network.clone(), self.hwnd)?);
        self.clock = Some(FrameClock::new(self.hwnd));
        self.clock
            .as_ref()
            .unwrap()
            .select_monitor(self.hwnd, false);
        self.send(true);
        self.resize()?;
        let _ = ShowWindow(self.hwnd, SW_SHOW);
        let _ = UpdateWindow(self.hwnd);
        if !self.options.windowed && !diagnostic {
            self.fullscreen_toggle()?;
        }
        if !self.options.profile.as_os_str().is_empty() {
            self.profile = Some(profile::Profile::new(now()));
            SetTimer(Some(self.hwnd), 2, 10, None);
        } else if diagnostic {
            SetTimer(Some(self.hwnd), 3, 500, None);
        }
        self.drain()?;
        Ok(())
    }
}
pub unsafe fn run(panel: Panel) -> Result<i32> {
    // The owner allocation remains fixed until its window has been destroyed.
    // No mutable panel borrow spans GetMessage/DispatchMessage or a modal loop.
    let owner = Box::new(std::cell::RefCell::new(panel));
    owner.borrow().ctx.owner.set(&*owner);
    owner.borrow_mut().create_window()?;
    let mut msg = MSG::default();
    loop {
        let result = GetMessageW(&mut msg, None, 0, 0).0;
        let error = owner.borrow().ctx.error.borrow_mut().take();
        if let Some(error) = error {
            return Err(error);
        }
        if result == -1 {
            bail!("Windows message loop failed");
        }
        if result == 0 {
            return Ok(msg.wParam.0 as i32);
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            self.ctx.owner.set(std::ptr::null());
            if let Some(c) = &mut self.clock {
                c.stop();
            }
            if let Some(s) = &mut self.sender {
                s.stop();
            }
            if !self.hwnd.is_invalid() && !self.destroyed {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}
