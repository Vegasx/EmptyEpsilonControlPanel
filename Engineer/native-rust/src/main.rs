#![windows_subsystem = "windows"]
mod frame_clock;
mod model;
mod panel;
mod profile;
mod render;
mod transport;
mod verification;
mod win;
use anyhow::{bail, ensure, Context as _, Result};
use model::*;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Instant,
};
use transport::{wide, Endpoint, Sender, TransportOptions};
use windows::{
    core::{w, PCWSTR},
    Win32::{Foundation::*, System::Com::*, UI::WindowsAndMessaging::*},
};
const NETWORK_CHANGED: u32 = WM_APP + 1;
const FRAME_READY: u32 = WM_APP + 2;
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let f = std::fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    ensure!(
        f.metadata()?.len() <= 1024 * 1024,
        "Configuration exceeds 1 MiB"
    );
    use std::io::Read;
    let mut bytes = Vec::new();
    f.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "Configuration exceeds 1 MiB");
    Ok(bytes)
}
fn write(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(p) = path.parent() {
        if !p.as_os_str().is_empty() {
            std::fs::create_dir_all(p)?;
        }
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
#[derive(Default)]
struct Options {
    config: PathBuf,
    transport: PathBuf,
    profile: PathBuf,
    screenshot: PathBuf,
    verify: PathBuf,
    network: TransportOptions,
    windowed: bool,
    reduced: bool,
}
impl Options {
    fn diagnostic(&self) -> bool {
        !self.profile.as_os_str().is_empty()
            || !self.screenshot.as_os_str().is_empty()
            || !self.verify.as_os_str().is_empty()
    }
}
fn network_test(endpoint: String, path: &Path, config: Configuration) -> Result<i32> {
    let mut state = State::new(config);
    let sender = Sender::new(
        TransportOptions {
            endpoint,
            send_hz: 30,
        },
        HWND::default(),
    )?;
    let mut seq = 1;
    sender.submit(state.payload(seq, false), seq, false);
    std::thread::sleep(std::time::Duration::from_millis(100));
    for i in 0..300 {
        state.set(0, i as f64);
        seq += 1;
        sender.submit(state.payload(seq, false), seq, false);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    state.set(0, 147.0);
    state.commit(0);
    seq += 1;
    sender.submit(state.payload(seq, true), seq, true);
    let deadline = Instant::now() + std::time::Duration::from_secs(15);
    while sender.counts().2 < seq && Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (sent, failed, ack) = sender.counts();
    let passed = ack == seq;
    write(
        path,
        &serde_json::json!({"passed":passed,"sent":sent,"failed":failed,"finalSequence":seq,"acknowledged":ack,"status":sender.status()}),
    )?;
    Ok(if passed { 0 } else { 1 })
}
fn startup(report: &mut PathBuf) -> Result<i32> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        struct Com;
        impl Drop for Com {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _com = Com;
        let base = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let mut o = Options {
            config: base.join("config.json"),
            transport: base.join("transport.json"),
            network: TransportOptions {
                endpoint: String::new(),
                send_hz: 30,
            },
            ..Default::default()
        };
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        let mut i = 0;
        let mut endpoint_override = false;
        let mut transport_override = false;
        let mut test = None;
        for pair in args.windows(2) {
            if pair[0] == "--verify" || pair[0] == "--profile" {
                *report = PathBuf::from(&pair[1]);
            }
        }
        while i < args.len() {
            let arg = args[i].to_string_lossy();
            let mut next = || -> Result<std::ffi::OsString> {
                i += 1;
                args.get(i)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Missing value for {arg}"))
            };
            match arg.as_ref() {
                "--windowed" => o.windowed = true,
                "--reduced-motion" => o.reduced = true,
                "--config" => o.config = PathBuf::from(next()?),
                "--transport" => {
                    o.transport = PathBuf::from(next()?);
                    transport_override = true;
                }
                "--endpoint" => {
                    o.network.endpoint = next()?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("Endpoint must be Unicode"))?;
                    endpoint_override = true;
                }
                "--profile" => o.profile = PathBuf::from(next()?),
                "--verify" => o.verify = PathBuf::from(next()?),
                "--screenshot" => o.screenshot = PathBuf::from(next()?),
                "--network-test" => {
                    let endpoint = next()?.to_string_lossy().into_owned();
                    let path = PathBuf::from(next()?);
                    *report = path.clone();
                    test = Some((endpoint, path));
                }
                _ => bail!("Unknown argument: {arg}"),
            }
            i += 1;
        }
        let config = Configuration::parse(&read(&o.config)?)
            .with_context(|| format!("Configuration {}", o.config.display()))?;
        if let Some((endpoint, path)) = test {
            return network_test(endpoint, &path, config);
        }
        if transport_override || o.transport.exists() {
            let t = parse_json(&read(&o.transport)?)?;
            let enabled = t["enabled"]
                .as_bool()
                .context("transport.enabled must be true or false")?;
            o.network.send_hz = integer(&t["sendHz"], 1, 120)? as u32;
            if enabled && !endpoint_override {
                o.network.endpoint = t["endpoint"]
                    .as_str()
                    .context("endpoint must be text")?
                    .into();
                Endpoint::parse(&o.network.endpoint)?;
            }
        }
        if endpoint_override {
            Endpoint::parse(&o.network.endpoint)?;
        }
        if o.diagnostic() {
            o.network.endpoint.clear();
        }
        panel::run(panel::Panel::new(config, o)?)
    }
}
fn main() {
    let mut report = PathBuf::new();
    let code = match startup(&mut report) {
        Ok(code) => code,
        Err(e) => {
            let text = format!("{e:#}");
            if !report.as_os_str().is_empty() {
                let _ = write(&report, &serde_json::json!({"passed":false,"error":text}));
            } else {
                unsafe {
                    let text = wide(&text);
                    MessageBoxW(
                        None,
                        PCWSTR(text.as_ptr()),
                        w!("Epsilon Engineering Rust — error"),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
            1
        }
    };
    std::process::exit(code);
}
