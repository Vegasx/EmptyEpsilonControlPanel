use serde_json::{json, Value};
use windows::Win32::{
    Foundation::FILETIME,
    System::{ProcessStatus::*, SystemInformation::*, Threading::*},
};
pub fn cpu_ms() -> f64 {
    unsafe {
        let (mut c, mut e, mut k, mut u) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let _ = GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u);
        (((k.dwHighDateTime as u64) << 32 | k.dwLowDateTime as u64)
            + ((u.dwHighDateTime as u64) << 32 | u.dwLowDateTime as u64)) as f64
            / 10000.0
    }
}
pub fn utc() -> String {
    unsafe {
        let t = GetSystemTime();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        )
    }
}
pub struct Profile {
    pub start: f64,
    pub phase: i32,
    pub phase_start: f64,
    pub cpu: f64,
    pub utc: String,
    pub draw_start: u64,
    pub actions: i32,
    pub resets: i32,
    pub warm_switches: i32,
    pub warm_updates: i32,
    pub warm_reset: bool,
    pub reports: Vec<Value>,
    pub intervals: Vec<f64>,
    pub previous_draw: f64,
}
impl Profile {
    pub fn new(start: f64) -> Self {
        Self {
            start,
            phase: -1,
            phase_start: 0.0,
            cpu: 0.0,
            utc: String::new(),
            draw_start: 0,
            actions: 0,
            resets: 0,
            warm_switches: 0,
            warm_updates: 0,
            warm_reset: false,
            reports: Vec::new(),
            intervals: Vec::new(),
            previous_draw: 0.0,
        }
    }
    pub fn begin_phase(&mut self, t: f64, draws: u64) {
        self.phase_start = t;
        self.cpu = cpu_ms();
        self.utc = utc();
        self.draw_start = draws;
        self.actions = 0;
        self.resets = 0;
        self.intervals.clear();
        self.previous_draw = 0.0;
    }
    pub fn record_draw(&mut self, t: f64) {
        if self.phase >= 0 && self.previous_draw != 0.0 {
            self.intervals.push(t - self.previous_draw);
        }
        self.previous_draw = t;
    }
    pub fn finish_phase(&mut self, t: f64, draws: u64) {
        unsafe {
            let mut m = PROCESS_MEMORY_COUNTERS_EX {
                cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                ..Default::default()
            };
            let _ = GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut m as *mut _ as _,
                size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            );
            self.intervals.sort_by(f64::total_cmp);
            let percentile = |f: f64| {
                self.intervals
                    .get(((self.intervals.len().saturating_sub(1)) as f64 * f) as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
            self.reports.push(json!({"name":(["idle","switching","settled","dragging"][self.phase as usize]),"start":self.utc,"end":utc(),"pids":[{"id":GetCurrentProcessId(),"role":"Rust Direct2D"}],"elapsedMs":t-self.phase_start,"actions":self.actions,"resets":self.resets,"cpuMs":cpu_ms()-self.cpu,"privateMiB":m.PrivateUsage as f64/1048576.0,"workingSetMiB":m.WorkingSetSize as f64/1048576.0,"draws":draws-self.draw_start,"drawIntervalMedianMs":percentile(0.5),"drawIntervalP95Ms":percentile(0.95)}));
        }
    }
}

// Opt-in workload parity with the optimized C++ executable. Normal launches do
// not create a Profile, keep frame samples, or run any workload timer.
impl crate::panel::Panel {
    pub unsafe fn profile_tick(&mut self) -> anyhow::Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let mut p = self.profile.take().unwrap();
        let t = crate::now();
        let elapsed = t - p.start;
        let mut due = p.start + 6000.0;
        if elapsed >= 6000.0 {
            let desired = ((elapsed - 6000.0) / 100.0).clamp(0.0, 30.0) as i32;
            while p.warm_switches < desired {
                p.warm_switches += 1;
                self.toggle();
                if p.warm_switches % 7 == 0 {
                    self.reset();
                }
            }
            due = p.start + 6000.0 + (p.warm_switches + 1) as f64 * 100.0;
            if elapsed >= 9000.0 {
                let desired = ((elapsed - 9000.0) / (2000.0 / 60.0)).clamp(0.0, 60.0) as i32;
                while p.warm_updates < desired {
                    p.warm_updates += 1;
                    self.state.set(
                        0,
                        ((0.5
                            + 0.48 * (p.warm_updates as f64 * std::f64::consts::PI / 30.0).sin())
                            * 300.0)
                            .round(),
                    );
                    self.update_motion(false);
                }
                due = p.start + 9000.0 + (p.warm_updates + 1) as f64 * (2000.0 / 60.0);
                if elapsed >= 11000.0 {
                    if !p.warm_reset {
                        self.state.set(0, 100.0);
                        self.reset();
                        p.warm_reset = true;
                    }
                    due = p.start + 17000.0;
                    if elapsed >= 17000.0 {
                        if p.phase == -1 {
                            p.phase = 0;
                            p.begin_phase(t, self.draws);
                        }
                        let durations = [5000.0, 12000.0, 5000.0, 10000.0];
                        let duration = durations[p.phase as usize];
                        let phase_elapsed = t - p.phase_start;
                        let count = if p.phase == 1 {
                            120
                        } else if p.phase == 3 {
                            300
                        } else {
                            0
                        };
                        let desired = count.min((phase_elapsed * count as f64 / duration) as i32);
                        while p.actions < desired {
                            p.actions += 1;
                            if p.phase == 1 {
                                self.toggle();
                                if p.actions % 7 == 0 {
                                    self.reset();
                                    p.resets += 1;
                                }
                            } else {
                                self.state.set(
                                    0,
                                    ((0.5
                                        + 0.48
                                            * (p.actions as f64 * std::f64::consts::PI / 30.0)
                                                .sin())
                                        * 300.0)
                                        .round(),
                                );
                                self.update_motion(false);
                            }
                        }
                        due = p.phase_start
                            + if count > 0 {
                                (p.actions + 1) as f64 * duration / count as f64
                            } else {
                                duration
                            };
                        if phase_elapsed >= duration {
                            p.finish_phase(t, self.draws);
                            p.phase += 1;
                            if p.phase == 4 {
                                crate::write(
                                    &self.options.profile,
                                    &json!({"implementation":"rust-direct2d-optimized","width":self.g.width,"height":self.g.height,"deviceScaleFactor":self.dpi/96.0,"logicalProcessors":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1),"reducedMotion":self.reduced,"transportEnabled":false,"frameMetric":"CPU draw callback intervals; not display presentation","phases":p.reports}),
                                )?;
                                let _ = KillTimer(Some(self.hwnd), 2);
                                DestroyWindow(self.hwnd)?;
                                return Ok(());
                            }
                            p.begin_phase(crate::now(), self.draws);
                            due = p.phase_start
                                + if p.phase == 1 {
                                    100.0
                                } else if p.phase == 3 {
                                    33.0
                                } else {
                                    durations[p.phase as usize]
                                };
                        }
                    }
                }
            }
        }
        self.profile = Some(p);
        SetTimer(
            Some(self.hwnd),
            2,
            (due - crate::now()).ceil().max(1.0) as u32,
            None,
        );
        Ok(())
    }
}
