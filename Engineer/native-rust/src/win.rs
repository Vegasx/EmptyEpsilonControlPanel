//! Copy messages into owned events, then dispatch under a checked mutable borrow.
//! Reentrant Win32 callbacks queue their work until that borrow ends. This also
//! keeps rendering live inside Windows' modal move/resize message loops.
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Input::Pointer::*, WindowsAndMessaging::*},
};
#[derive(Clone)]
pub enum Event {
    Paint(Vec<RECT>),
    Resize(bool),
    Move,
    Display,
    Dpi(f32, RECT),
    Timer(usize),
    Mouse(u32, f32, f32),
    Pointer(u32, u32, u32, f32, f32),
    Capture(u32),
    Cancel,
    Key(usize),
    Blur,
    Preference,
    Network,
    Frame,
    Destroy,
}
pub struct Context {
    pub queue: RefCell<VecDeque<Event>>,
    pub dpi: Cell<f32>,
    pub owner: Cell<*const RefCell<crate::panel::Panel>>,
    pub error: RefCell<Option<anyhow::Error>>,
}
impl Context {
    pub fn new() -> Self {
        Self {
            queue: RefCell::new(VecDeque::new()),
            dpi: Cell::new(96.0),
            owner: Cell::new(std::ptr::null()),
            error: RefCell::new(None),
        }
    }
}
pub unsafe fn client_rect(h: HWND, r: &mut RECT) -> windows::core::Result<()> {
    GetClientRect(h, r)
}
pub unsafe fn update_rectangles(hwnd: HWND) -> Vec<RECT> {
    let region = CreateRectRgn(0, 0, 0, 0);
    let mut result = Vec::new();
    if GetUpdateRgn(hwnd, region, false) != GDI_REGION_TYPE(0) {
        let size = GetRegionData(region, 0, None);
        if size > 0 {
            let mut words = vec![0u32; (size as usize).div_ceil(4)];
            let p = words.as_mut_ptr() as *mut RGNDATA;
            if GetRegionData(region, size, Some(p)) > 0 {
                result.extend_from_slice(std::slice::from_raw_parts(
                    (*p).Buffer.as_ptr() as *const RECT,
                    (*p).rdh.nCount as usize,
                ));
            }
        }
    }
    let _ = DeleteObject(region.into());
    result
}
pub unsafe fn union_rectangles(rectangles: &[RECT]) -> Vec<RECT> {
    let region = CreateRectRgn(0, 0, 0, 0);
    for r in rectangles {
        let part = CreateRectRgnIndirect(r);
        CombineRgn(Some(region), Some(region), Some(part), RGN_OR);
        let _ = DeleteObject(part.into());
    }
    let size = GetRegionData(region, 0, None);
    let mut result = Vec::new();
    if size > 0 {
        let mut words = vec![0u32; (size as usize).div_ceil(4)];
        let p = words.as_mut_ptr() as *mut RGNDATA;
        if GetRegionData(region, size, Some(p)) > 0 {
            result.extend_from_slice(std::slice::from_raw_parts(
                (*p).Buffer.as_ptr() as *const RECT,
                (*p).rdh.nCount as usize,
            ));
        }
    }
    let _ = DeleteObject(region.into());
    result
}
pub unsafe extern "system" fn procedure(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let mut ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *const Context;
    if msg == WM_NCCREATE {
        ptr = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams as _;
        SetWindowLongPtrW(h, GWLP_USERDATA, ptr as isize);
    }
    if ptr.is_null() {
        return DefWindowProcW(h, msg, wp, lp);
    }
    let ctx = &*ptr;
    let x = (lp.0 as u16 as i16) as f32 * 96.0 / ctx.dpi.get();
    let y = ((lp.0 >> 16) as u16 as i16) as f32 * 96.0 / ctx.dpi.get();
    let e = match msg {
        WM_PAINT => {
            let exposed = update_rectangles(h);
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(h, &mut ps);
            let _ = EndPaint(h, &ps);
            Some(Event::Paint(exposed))
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_SIZE => Some(Event::Resize(wp.0 == SIZE_MINIMIZED as usize)),
        WM_MOVE => Some(Event::Move),
        WM_DISPLAYCHANGE => Some(Event::Display),
        WM_DPICHANGED => {
            let dpi = ((wp.0 >> 16) & 65535) as f32;
            ctx.dpi.set(dpi);
            Some(Event::Dpi(dpi, *(lp.0 as *const RECT)))
        }
        WM_GETMINMAXINFO => {
            let m = &mut *(lp.0 as *mut MINMAXINFO);
            m.ptMinTrackSize = POINT {
                x: (640.0 * ctx.dpi.get() / 96.0) as i32,
                y: (480.0 * ctx.dpi.get() / 96.0) as i32,
            };
            return LRESULT(0);
        }
        WM_TIMER => Some(Event::Timer(wp.0)),
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP => Some(Event::Mouse(msg, x, y)),
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
            let id = (wp.0 & 65535) as u32;
            let mut info = POINTER_INFO::default();
            if GetPointerInfo(id, &mut info).is_err() || info.pointerType == PT_MOUSE {
                None
            } else {
                let mut p = info.ptPixelLocation;
                let _ = ScreenToClient(h, &mut p);
                Some(Event::Pointer(
                    msg,
                    id,
                    info.pointerFlags.0,
                    p.x as f32 * 96.0 / ctx.dpi.get(),
                    p.y as f32 * 96.0 / ctx.dpi.get(),
                ))
            }
        }
        WM_POINTERCAPTURECHANGED => Some(Event::Capture((wp.0 & 65535) as u32)),
        WM_CAPTURECHANGED => Some(Event::Capture(0)),
        WM_CANCELMODE => Some(Event::Cancel),
        WM_KEYDOWN => Some(Event::Key(wp.0)),
        WM_KILLFOCUS => Some(Event::Blur),
        WM_ACTIVATE if wp.0 & 65535 == WA_INACTIVE as usize => Some(Event::Blur),
        WM_SETTINGCHANGE => Some(Event::Preference),
        crate::NETWORK_CHANGED => Some(Event::Network),
        crate::FRAME_READY => Some(Event::Frame),
        WM_DESTROY => {
            ctx.queue.borrow_mut().clear();
            Some(Event::Destroy)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(h, GWLP_USERDATA, 0);
            return DefWindowProcW(h, msg, wp, lp);
        }
        _ => return DefWindowProcW(h, msg, wp, lp),
    };
    if let Some(e) = e {
        ctx.queue.borrow_mut().push_back(e);
        let owner = ctx.owner.get();
        if !owner.is_null() {
            let result = match (&*owner).try_borrow_mut() {
                Ok(mut panel) => panel.drain(),
                Err(_) => return LRESULT(0),
            };
            if let Err(error) = result {
                *ctx.error.borrow_mut() = Some(error);
                let _ = DestroyWindow(h);
                PostQuitMessage(1);
            }
        }
    }
    LRESULT(0)
}
