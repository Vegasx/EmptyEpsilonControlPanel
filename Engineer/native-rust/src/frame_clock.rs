//! One coalesced notification per monitor vblank; no polling while idle.
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    Graphics::{Dxgi::*, Gdi::*},
    System::Com::*,
    UI::WindowsAndMessaging::PostMessageW,
};
struct Gate {
    enabled: bool,
    stopping: bool,
    pending: bool,
    monitor: usize,
    generation: u64,
}
struct Shared {
    gate: Mutex<Gate>,
    wake: Condvar,
    vblanks: AtomicU64,
    fallbacks: AtomicU64,
}
pub struct FrameClock {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl FrameClock {
    pub fn new(hwnd: HWND) -> Self {
        let shared = Arc::new(Shared {
            gate: Mutex::new(Gate {
                enabled: false,
                stopping: false,
                pending: false,
                monitor: 0,
                generation: 0,
            }),
            wake: Condvar::new(),
            vblanks: AtomicU64::new(0),
            fallbacks: AtomicU64::new(0),
        });
        let s = shared.clone();
        let window = hwnd.0 as usize;
        let worker = thread::Builder::new()
            .name("Display pacing".into())
            .spawn(move || unsafe {
                let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
                let mut output: Option<IDXGIOutput> = None;
                let mut generation = u64::MAX;
                let mut hz = 60.0;
                let mut q = s.gate.lock().unwrap();
                loop {
                    while !q.stopping && (!q.enabled || q.pending) {
                        q = s.wake.wait(q).unwrap();
                    }
                    if q.stopping {
                        break;
                    }
                    if generation != q.generation {
                        generation = q.generation;
                        let monitor = q.monitor;
                        drop(q);
                        (output, hz) = select(monitor);
                        q = s.gate.lock().unwrap();
                    }
                    drop(q);
                    let success = output.as_ref().is_some_and(|o| o.WaitForVBlank().is_ok());
                    q = s.gate.lock().unwrap();
                    if success {
                        s.vblanks.fetch_add(1, Ordering::Relaxed);
                    } else {
                        s.fallbacks.fetch_add(1, Ordering::Relaxed);
                        let (next, _) = s
                            .wake
                            .wait_timeout_while(q, Duration::from_secs_f64(1.0 / hz), |g| {
                                !g.stopping && g.enabled
                            })
                            .unwrap();
                        q = next;
                    }
                    if q.stopping {
                        break;
                    }
                    if !q.enabled {
                        continue;
                    }
                    q.pending = true;
                    if PostMessageW(
                        Some(HWND(window as _)),
                        super::FRAME_READY,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .is_err()
                    {
                        q.pending = false;
                    }
                }
                drop(output);
                if initialized {
                    CoUninitialize();
                }
            })
            .expect("Cannot start display clock");
        Self {
            shared,
            worker: Some(worker),
        }
    }
    pub fn active(&self, value: bool) {
        self.shared.gate.lock().unwrap().enabled = value;
        self.shared.wake.notify_all();
    }
    pub fn consumed(&self) {
        self.shared.gate.lock().unwrap().pending = false;
        self.shared.wake.notify_all();
    }
    pub fn is_active(&self) -> bool {
        self.shared.gate.lock().unwrap().enabled
    }
    pub fn select_monitor(&self, hwnd: HWND, force: bool) {
        unsafe {
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST).0 as usize;
            let mut q = self.shared.gate.lock().unwrap();
            if force || q.monitor != monitor {
                q.monitor = monitor;
                q.generation += 1;
                self.shared.wake.notify_all();
            }
        }
    }
    pub fn counts(&self) -> (u64, u64) {
        (
            self.shared.vblanks.load(Ordering::Relaxed),
            self.shared.fallbacks.load(Ordering::Relaxed),
        )
    }
    pub fn stop(&mut self) {
        self.shared.gate.lock().unwrap().stopping = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.worker.take() {
            let _ = t.join();
        }
    }
}
impl Drop for FrameClock {
    fn drop(&mut self) {
        self.stop();
    }
}
unsafe fn select(monitor: usize) -> (Option<IDXGIOutput>, f64) {
    let mut chosen = None;
    if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
        let mut i = 0;
        'outer: while let Ok(a) = factory.EnumAdapters1(i) {
            let mut j = 0;
            while let Ok(o) = a.EnumOutputs(j) {
                if o.GetDesc().is_ok_and(|d| d.Monitor.0 as usize == monitor) {
                    chosen = Some(o);
                    break 'outer;
                }
                j += 1;
            }
            i += 1;
        }
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let mut hz = 60.0;
    if GetMonitorInfoW(HMONITOR(monitor as _), &mut info as *mut _ as _).as_bool() {
        let mut mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        if EnumDisplaySettingsW(
            windows::core::PCWSTR(info.szDevice.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
        .as_bool()
            && mode.dmDisplayFrequency > 1
        {
            hz = mode.dmDisplayFrequency as f64;
        }
    }
    (chosen, hz)
}
