//! Single worker owns all WinHTTP handles. At most one in-flight request and
//! one newest pending full state; final submissions bypass the move rate limit.
use anyhow::{ensure, Result};
use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Networking::WinHttp::*,
        UI::WindowsAndMessaging::PostMessageW,
    },
};
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Endpoint {
    host: Vec<u16>,
    path: Vec<u16>,
    port: u16,
    secure: bool,
}
impl Endpoint {
    pub fn parse(url: &str) -> Result<Self> {
        unsafe {
            ensure!(
                !url.is_empty() && !url.contains(['\0', '#']),
                "Endpoint must be HTTP/HTTPS without credentials or fragment"
            );
            let text: Vec<u16> = url.encode_utf16().collect();
            let mut u = URL_COMPONENTS {
                dwStructSize: size_of::<URL_COMPONENTS>() as u32,
                dwHostNameLength: u32::MAX,
                dwUrlPathLength: u32::MAX,
                dwExtraInfoLength: u32::MAX,
                dwUserNameLength: u32::MAX,
                dwPasswordLength: u32::MAX,
                ..Default::default()
            };
            WinHttpCrackUrl(&text, 0, &mut u)?;
            ensure!(
                (u.nScheme == WINHTTP_INTERNET_SCHEME_HTTP
                    || u.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS)
                    && u.dwHostNameLength > 0
                    && u.dwUserNameLength == 0
                    && u.dwPasswordLength == 0,
                "Endpoint must be HTTP/HTTPS without credentials or fragment"
            );
            let mut host =
                std::slice::from_raw_parts(u.lpszHostName.0, u.dwHostNameLength as usize).to_vec();
            host.push(0);
            let mut path = if u.dwUrlPathLength > 0 {
                std::slice::from_raw_parts(u.lpszUrlPath.0, u.dwUrlPathLength as usize).to_vec()
            } else {
                vec![b'/' as u16]
            };
            if u.dwExtraInfoLength > 0 {
                path.extend_from_slice(std::slice::from_raw_parts(
                    u.lpszExtraInfo.0,
                    u.dwExtraInfoLength as usize,
                ));
            }
            path.push(0);
            Ok(Self {
                host,
                path,
                port: u.nPort,
                secure: u.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS,
            })
        }
    }
}
struct Handle(*mut c_void);
impl Handle {
    fn new(p: *mut c_void) -> Result<Self> {
        ensure!(!p.is_null(), "WinHTTP: {}", std::io::Error::last_os_error());
        Ok(Self(p))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}
#[derive(Clone, Default)]
pub struct TransportOptions {
    pub endpoint: String,
    pub send_hz: u32,
}
struct Delivery {
    json: String,
    sequence: u64,
    final_state: bool,
}
struct Queue {
    pending: Option<Delivery>,
    stopping: bool,
    status: String,
}
struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    pub sent: AtomicU64,
    pub failed: AtomicU64,
    pub acknowledged: AtomicU64,
    hwnd: usize,
}
pub struct Sender {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    enabled: bool,
}
impl Sender {
    pub fn new(options: TransportOptions, hwnd: HWND) -> Result<Self> {
        ensure!(
            (1..=120).contains(&options.send_hz),
            "sendHz must be 1..120"
        );
        let enabled = !options.endpoint.is_empty();
        if enabled {
            Endpoint::parse(&options.endpoint)?;
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: None,
                stopping: false,
                status: if enabled { "CONNECTING" } else { "LOCAL" }.into(),
            }),
            wake: Condvar::new(),
            sent: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            acknowledged: AtomicU64::new(0),
            hwnd: hwnd.0 as usize,
        });
        let worker = if enabled {
            let s = shared.clone();
            Some(
                thread::Builder::new()
                    .name("HTTP delivery".into())
                    .spawn(move || {
                        if let Err(e) = run(&s, options) {
                            status(&s, format!("HTTP ERROR: {e}"));
                        }
                    })?,
            )
        } else {
            None
        };
        Ok(Self {
            shared,
            worker,
            enabled,
        })
    }
    pub fn submit(&self, json: String, sequence: u64, final_state: bool) {
        if self.enabled {
            let mut q = self.shared.queue.lock().unwrap();
            if !q.stopping {
                q.pending = Some(Delivery {
                    json,
                    sequence,
                    final_state,
                });
                self.shared.wake.notify_one();
            }
        }
    }
    pub fn status(&self) -> String {
        self.shared.queue.lock().unwrap().status.clone()
    }
    pub fn counts(&self) -> (u64, u64, u64) {
        (
            self.shared.sent.load(Ordering::Relaxed),
            self.shared.failed.load(Ordering::Relaxed),
            self.shared.acknowledged.load(Ordering::Acquire),
        )
    }
    pub fn stop(&mut self) {
        self.shared.queue.lock().unwrap().stopping = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.worker.take() {
            let _ = t.join();
        }
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        self.stop();
    }
}
fn status(s: &Shared, next: String) {
    let changed = {
        let mut q = s.queue.lock().unwrap();
        let changed = q.status != next;
        q.status = next;
        changed
    };
    if changed && s.hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(s.hwnd as _)),
                super::NETWORK_CHANGED,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}
unsafe fn post(connection: &Handle, e: &Endpoint, d: &Delivery) -> Result<()> {
    let r = Handle::new(WinHttpOpenRequest(
        connection.0,
        w!("POST"),
        PCWSTR(e.path.as_ptr()),
        PCWSTR::null(),
        PCWSTR::null(),
        std::ptr::null(),
        if e.secure {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        },
    ))?;
    WinHttpSetOption(
        Some(r.0),
        WINHTTP_OPTION_REDIRECT_POLICY,
        Some(&WINHTTP_OPTION_REDIRECT_POLICY_NEVER.to_ne_bytes()),
    )?;
    let _ = WinHttpSetOption(
        Some(r.0),
        WINHTTP_OPTION_AUTOLOGON_POLICY,
        Some(&WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH.to_ne_bytes()),
    );
    let headers: Vec<u16> =
        "Content-Type: application/json; charset=utf-8\r\nAccept: application/json\r\n"
            .encode_utf16()
            .collect();
    WinHttpSendRequest(
        r.0,
        Some(&headers),
        Some(d.json.as_ptr() as _),
        d.json.len() as u32,
        d.json.len() as u32,
        0,
    )?;
    WinHttpReceiveResponse(r.0, null_mut())?;
    let mut code = 0u32;
    let mut size = 4u32;
    WinHttpQueryHeaders(
        r.0,
        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        PCWSTR::null(),
        Some(&mut code as *mut _ as _),
        &mut size,
        null_mut(),
    )?;
    ensure!((200..300).contains(&code), "HTTP {code}");
    let mut buffer = [0u8; 4096];
    let mut total = 0;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut read = 0;
        if WinHttpReadData(
            r.0,
            buffer.as_mut_ptr() as _,
            buffer.len() as u32,
            &mut read,
        )
        .is_err()
        {
            break;
        }
        total += read;
        if read == 0 || total >= 65536 || Instant::now() >= deadline {
            break;
        }
    }
    Ok(())
}
fn run(s: &Shared, options: TransportOptions) -> Result<()> {
    unsafe {
        let e = Endpoint::parse(&options.endpoint)?;
        let session = Handle::new(WinHttpOpen(
            w!("Epsilon-Rust/0.1"),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 1000, 1000, 1000, 1000)?;
        let connection = Handle::new(WinHttpConnect(
            session.0,
            PCWSTR(e.host.as_ptr()),
            e.port,
            0,
        ))?;
        let mut next_send = Instant::now();
        let mut retry_after = next_send;
        let mut retry_ms = 250;
        loop {
            let d = {
                let mut q = s.queue.lock().unwrap();
                loop {
                    if q.stopping {
                        return Ok(());
                    }
                    if let Some(p) = q.pending.as_ref() {
                        let due = if p.final_state {
                            retry_after
                        } else {
                            retry_after.max(next_send)
                        };
                        if Instant::now() >= due {
                            break q.pending.take().unwrap();
                        }
                        q = s
                            .wake
                            .wait_timeout(q, due.saturating_duration_since(Instant::now()))
                            .unwrap()
                            .0;
                    } else {
                        q = s.wake.wait(q).unwrap();
                    }
                }
            };
            match post(&connection, &e, &d) {
                Ok(()) => {
                    s.sent.fetch_add(1, Ordering::Relaxed);
                    s.acknowledged.store(d.sequence, Ordering::Release);
                    retry_ms = 250;
                    retry_after = Instant::now();
                    next_send = retry_after
                        + Duration::from_nanos(1_000_000_000u64.div_ceil(options.send_hz as u64));
                    status(s, "HTTP OK".into());
                }
                Err(e) => {
                    s.failed.fetch_add(1, Ordering::Relaxed);
                    {
                        let mut q = s.queue.lock().unwrap();
                        if q.pending.is_none() {
                            q.pending = Some(d);
                        }
                    }
                    retry_after = Instant::now() + Duration::from_millis(retry_ms);
                    retry_ms = (retry_ms * 2).min(4000);
                    status(s, format!("RETRY: {e}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_and_rate_validation() {
        for url in [
            "file:///bad",
            "ftp://server/x",
            "http://user:pass@server/",
            "http://server/#fragment",
            "http://server/\0bad",
            "",
        ] {
            assert!(Endpoint::parse(url).is_err(), "{url:?}");
        }
        for url in [
            "http://127.0.0.1:4181/controller?station=engineer",
            "https://example.com/controller",
            "http://[::1]:4181/",
        ] {
            assert!(Endpoint::parse(url).is_ok(), "{url}");
        }
        assert!(Sender::new(
            TransportOptions {
                endpoint: String::new(),
                send_hz: 0
            },
            HWND::default()
        )
        .is_err());
        assert!(Sender::new(
            TransportOptions {
                endpoint: String::new(),
                send_hz: 121
            },
            HWND::default()
        )
        .is_err());
    }
}
