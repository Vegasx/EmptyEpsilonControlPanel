//! Retained Direct2D renderer. Static artwork and tintable A8 masks are cached;
//! separate changed regions restore old pixels before drawing new visuals.
use crate::model::LABELS;
use anyhow::Result;
use std::{collections::HashMap, path::Path};
use windows::{
    core::{w, Interface, PCWSTR},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::*,
            Imaging::*,
        },
        System::Com::*,
    },
};
use windows_numerics::{Matrix3x2, Vector2};
pub type Rect = D2D_RECT_F;
pub type Color = D2D1_COLOR_F;
pub fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
    Rect {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}
pub fn color(rgb: u32, a: f32) -> Color {
    Color {
        r: ((rgb >> 16) & 255) as f32 / 255.0,
        g: ((rgb >> 8) & 255) as f32 / 255.0,
        b: (rgb & 255) as f32 / 255.0,
        a,
    }
}
pub fn mix(a: Color, b: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0) as f32;
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}
fn same(a: Color, b: Color) -> bool {
    a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
}
pub fn inside(r: Rect, x: f32, y: f32) -> bool {
    x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}
fn intersects(a: Rect, b: Rect) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}
fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}
fn matrix(scale: f32, x: f32, y: f32) -> Matrix3x2 {
    Matrix3x2 {
        M11: scale,
        M12: 0.0,
        M21: 0.0,
        M22: scale,
        M31: x,
        M32: y,
    }
}
fn rounded(r: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: r,
        radiusX: radius,
        radiusY: radius,
    }
}
fn ellipse(x: f32, y: f32, r: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: point(x, y),
        radiusX: r,
        radiusY: r,
    }
}
#[derive(Clone, Copy, Default)]
pub struct Geometry {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub right: f32,
    pub top: f32,
    pub heading: f32,
    pub ruler_y: f32,
    pub body_top: f32,
    pub row: f32,
    pub bottom: f32,
    pub slider_left: f32,
    pub slider_right: f32,
    pub grip_size: f32,
    pub reset: Rect,
    pub toggle: Rect,
}
impl Geometry {
    pub fn new(w: f32, h: f32) -> Self {
        let margin = (w * 0.03).clamp(20.0, 56.0);
        let right = w - margin;
        let top = (h * 0.027).clamp(16.0, 32.0);
        let heading = (w * 0.058).min(h * 0.086).clamp(28.0, 96.0);
        let header_bottom = top + heading * 1.25 + (h * 0.013).clamp(6.0, 16.0);
        let ruler_y = header_bottom + (h * 0.017).clamp(6.0, 18.0);
        let body_top = ruler_y + (h * 0.032).clamp(16.0, 30.0) + 4.0;
        let footer_height = (h * 0.081).clamp(44.0, 78.0) + (h * 0.014).clamp(6.0, 14.0);
        let bottom = h - top - footer_height;
        let row = ((bottom - body_top) / 8.0).max(20.0);
        let usable = w - 2.0 * margin;
        let slider_left = margin + usable * 0.42;
        let slider_right = slider_left + usable * 0.48;
        let grip_size = (h * 0.078).clamp(30.0, 84.0).min(row * 0.98);
        let foot_y = bottom + 12.0;
        let foot_bottom = h - top;
        let reset = rect(margin, foot_y, margin + usable * 0.3, foot_bottom);
        let mode_width = (usable * 0.62).min((w * 0.026).min(h * 0.048).clamp(18.0, 42.0) * 12.5);
        let mode_left = right - mode_width;
        Self {
            width: w,
            height: h,
            margin,
            right,
            top,
            heading,
            ruler_y,
            body_top,
            row,
            bottom,
            slider_left,
            slider_right,
            grip_size,
            reset,
            toggle: rect(
                mode_left + mode_width * 0.36,
                foot_y,
                mode_left + mode_width * 0.60,
                foot_bottom,
            ),
        }
    }
    pub fn full(&self) -> Rect {
        rect(0.0, 0.0, self.width, self.height)
    }
    pub fn row_bounds(&self, i: usize) -> Rect {
        let over = (self.slider_right - self.slider_left - self.grip_size).max(0.0) * 0.11 + 3.0;
        rect(
            self.slider_left - over,
            self.body_top + i as f32 * self.row - 2.0,
            self.right + 3.0,
            self.body_top + (i + 1) as f32 * self.row + 2.0,
        )
    }
    pub fn ruler_bounds(&self) -> Rect {
        rect(
            self.slider_left - 3.0,
            self.ruler_y - 3.0,
            self.slider_right + 3.0,
            self.body_top + 1.0,
        )
    }
    pub fn footer_bounds(&self) -> Rect {
        rect(
            self.margin - 3.0,
            self.reset.top - 3.0,
            self.right + 3.0,
            self.reset.bottom + 3.0,
        )
    }
    pub fn status_bounds(&self) -> Rect {
        rect(
            self.right - 193.0,
            self.top - 2.0,
            self.right + 2.0,
            self.top + 19.0,
        )
    }
    pub fn physical(&self, r: Rect, dpi: f32) -> RECT {
        RECT {
            left: (r.left.max(0.0) * dpi / 96.0).floor() as i32,
            top: (r.top.max(0.0) * dpi / 96.0).floor() as i32,
            right: (r.right.min(self.width) * dpi / 96.0).ceil() as i32,
            bottom: (r.bottom.min(self.height) * dpi / 96.0).ceil() as i32,
        }
    }
    fn footer(&self) -> (f32, f32, f32, f32, f32, Rect) {
        let d = (self.height * 0.081)
            .clamp(44.0, 78.0)
            .min(self.reset.bottom - self.reset.top);
        let cy = (self.reset.top + self.reset.bottom) / 2.0;
        let size = (self.width * 0.026)
            .min(self.height * 0.048)
            .clamp(18.0, 42.0);
        let mw = ((self.width - 2.0 * self.margin) * 0.62).min(size * 12.5);
        let ml = self.right - mw;
        let tw = (self.toggle.right - self.toggle.left).min(82.0);
        let th = (self.toggle.bottom - self.toggle.top).min(58.0);
        let cx = (self.toggle.left + self.toggle.right) / 2.0;
        (
            d,
            cy,
            mw,
            ml,
            size,
            rect(cx - tw / 2.0, cy - th / 2.0, cx + tw / 2.0, cy + th / 2.0),
        )
    }
}
#[derive(Clone)]
pub struct Visual {
    pub positions: [f64; 8],
    pub readouts: [String; 8],
    pub ruler: [String; 4],
    pub accent: Color,
    pub rail: Color,
    pub cooling: bool,
    pub lever: f64,
    pub focus: i32,
    pub status: String,
}
pub fn changed(g: &Geometry, old: Option<&Visual>, v: &Visual, full: bool) -> Vec<Rect> {
    let Some(o) = old.filter(|_| !full) else {
        return vec![g.full()];
    };
    let colors = !same(v.accent, o.accent) || !same(v.rail, o.rail);
    let mut dirty = Vec::with_capacity(11);
    for i in 0..8 {
        if colors
            || v.positions[i] != o.positions[i]
            || v.readouts[i] != o.readouts[i]
            || (v.focus == i as i32) != (o.focus == i as i32)
        {
            dirty.push(g.row_bounds(i));
        }
    }
    if v.ruler != o.ruler {
        dirty.push(g.ruler_bounds());
    }
    if colors
        || v.cooling != o.cooling
        || v.lever != o.lever
        || ((v.focus >= 8 || o.focus >= 8) && v.focus != o.focus)
    {
        dirty.push(g.footer_bounds());
    }
    if v.status != o.status {
        dirty.push(g.status_bounds());
    }
    dirty
}
struct Mask {
    image: ID2D1Bitmap,
    used: u64,
}
pub struct Graphics {
    pub factory: ID2D1Factory,
    text_factory: IDWriteFactory5,
    font_loader: IDWriteInMemoryFontFileLoader,
    fonts: IDWriteFontCollection1,
    pub imaging: IWICImagingFactory,
    pixels: IWICFormatConverter,
    pub target: Option<ID2D1HwndRenderTarget>,
    grip: Option<ID2D1Bitmap>,
    brush: Option<ID2D1SolidColorBrush>,
    paper: Option<ID2D1LinearGradientBrush>,
    backdrop: Option<ID2D1Bitmap>,
    owner: usize,
    layouts: HashMap<(String, u32), IDWriteTextLayout>,
    masks: HashMap<String, Mask>,
    serial: u64,
    pub background_builds: u64,
    pub mask_builds: u64,
    pub full: bool,
}
impl Graphics {
    pub unsafe fn new() -> Result<Self> {
        let factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let text_factory: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let font_loader = text_factory.CreateInMemoryFontFileLoader()?;
        text_factory.RegisterFontFileLoader(&font_loader)?;
        let font = include_bytes!("../../RussoOne-Regular.ttf");
        let file = font_loader.CreateInMemoryFontFileReference(
            &text_factory,
            font.as_ptr() as _,
            font.len() as u32,
            None,
        )?;
        let builder = text_factory.CreateFontSetBuilder()?;
        builder.AddFontFile(&file)?;
        let set = builder.CreateFontSet()?;
        let fonts = text_factory.CreateFontCollectionFromFontSet(&set)?;
        let imaging: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = imaging.CreateStream()?;
        stream.InitializeFromMemory(include_bytes!(
            "../../assets/slider-candidates/02-brass-grip.png"
        ))?;
        let decoder = imaging.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnLoad,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut iw, mut ih) = (0, 0);
        frame.GetSize(&mut iw, &mut ih)?;
        let scaler = imaging.CreateBitmapScaler()?;
        scaler.Initialize(
            &frame,
            192,
            (ih * 192 / iw).max(1),
            WICBitmapInterpolationModeFant,
        )?;
        let pixels = imaging.CreateFormatConverter()?;
        pixels.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        Ok(Self {
            factory,
            text_factory,
            font_loader,
            fonts,
            imaging,
            pixels,
            target: None,
            grip: None,
            brush: None,
            paper: None,
            backdrop: None,
            owner: 0,
            layouts: HashMap::new(),
            masks: HashMap::new(),
            serial: 0,
            background_builds: 0,
            mask_builds: 0,
            full: true,
        })
    }
    pub fn clear(&mut self) {
        self.masks.clear();
        self.backdrop = None;
        self.paper = None;
        self.brush = None;
        self.owner = 0;
        self.full = true;
    }
    pub unsafe fn resize(&mut self, w: u32, h: u32) {
        self.layouts.clear();
        self.clear();
        if let Some(t) = &self.target {
            if w > 0
                && h > 0
                && t.Resize(&D2D_SIZE_U {
                    width: w,
                    height: h,
                })
                .is_err()
            {
                self.discard();
            }
        }
    }
    pub fn discard(&mut self) {
        self.clear();
        self.grip = None;
        self.target = None;
    }
    fn properties(dpi: f32) -> D2D1_RENDER_TARGET_PROPERTIES {
        D2D1_RENDER_TARGET_PROPERTIES {
            dpiX: dpi,
            dpiY: dpi,
            ..Default::default()
        }
    }
    pub unsafe fn device(&mut self, hwnd: HWND, dpi: f32) -> Result<()> {
        if self.target.is_some() {
            return Ok(());
        }
        let mut r = RECT::default();
        crate::win::client_rect(hwnd, &mut r)?;
        if r.right <= 0 || r.bottom <= 0 {
            return Ok(());
        }
        let t = self.factory.CreateHwndRenderTarget(
            &Self::properties(dpi),
            &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U {
                    width: r.right as u32,
                    height: r.bottom as u32,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS
                    | D2D1_PRESENT_OPTIONS_IMMEDIATELY,
            },
        )?;
        self.grip = Some(t.CreateBitmapFromWicBitmap(&self.pixels, None)?);
        self.target = Some(t);
        Ok(())
    }
    unsafe fn ink(&mut self, rt: &ID2D1RenderTarget, c: Color) -> Result<ID2D1SolidColorBrush> {
        if let Some(b) = &self.brush {
            b.SetColor(&c);
        } else {
            self.brush = Some(rt.CreateSolidColorBrush(&c, None)?);
        }
        Ok(self.brush.as_ref().unwrap().clone())
    }
    unsafe fn fill(&mut self, rt: &ID2D1RenderTarget, r: Rect, c: Color) -> Result<()> {
        rt.FillRectangle(&r, &self.ink(rt, c)?);
        Ok(())
    }
    unsafe fn line(
        &mut self,
        rt: &ID2D1RenderTarget,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        c: Color,
        thickness: f32,
    ) -> Result<()> {
        rt.DrawLine(
            point(x1, y1),
            point(x2, y2),
            &self.ink(rt, c)?,
            thickness,
            None,
        );
        Ok(())
    }
    unsafe fn layout(&mut self, s: &str, size: f32) -> Result<IDWriteTextLayout> {
        let key = (s.to_string(), (size * 100.0) as u32);
        if let Some(l) = self.layouts.get(&key) {
            return Ok(l.clone());
        }
        let f = self.text_factory.CreateTextFormat(
            w!("Russo One"),
            &self.fonts,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-US"),
        )?;
        f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let l = self.text_factory.CreateTextLayout(
            &s.encode_utf16().collect::<Vec<_>>(),
            &f,
            4096.0,
            512.0,
        )?;
        if self.layouts.len() > 512 {
            self.layouts.clear();
        }
        self.layouts.insert(key, l.clone());
        Ok(l)
    }
    unsafe fn text(
        &mut self,
        rt: &ID2D1RenderTarget,
        s: &str,
        size: f32,
        b: Rect,
        c: Color,
        align: i32,
        dpi: f32,
        cached: bool,
    ) -> Result<()> {
        let l = self.layout(s, size)?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        l.GetMetrics(&mut metrics)?;
        let scale = 1.0f32
            .min((b.right - b.left) / metrics.widthIncludingTrailingWhitespace.max(1.0))
            .min((b.bottom - b.top) / metrics.height.max(1.0));
        let x = b.left
            + if align == 2 {
                b.right - b.left - metrics.widthIncludingTrailingWhitespace * scale
            } else if align == 1 {
                (b.right - b.left - metrics.widthIncludingTrailingWhitespace * scale) / 2.0
            } else {
                0.0
            };
        let y = b.top + (b.bottom - b.top - metrics.height * scale) / 2.0;
        if !cached {
            rt.SetTransform(&matrix(scale, x, y));
            rt.DrawTextLayout(
                point(0.0, 0.0),
                &l,
                &self.ink(rt, c)?,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
            rt.SetTransform(&matrix(1.0, 0.0, 0.0));
            return Ok(());
        }
        let left = (x * dpi / 96.0).floor() * 96.0 / dpi - 3.0;
        let top = (y * dpi / 96.0).floor() * 96.0 / dpi - 3.0;
        let key = format!(
            "text|{s}|{}|{scale:.6}|{:.6}|{:.6}",
            (size * 100.0) as u32,
            x - left,
            y - top
        );
        let image = self.mask(
            rt,
            key,
            metrics.widthIncludingTrailingWhitespace * scale + 8.0,
            metrics.height * scale + 8.0,
            dpi,
            |canvas, white| {
                canvas.SetTransform(&matrix(scale, x - left, y - top));
                canvas.DrawTextLayout(point(0.0, 0.0), &l, white, D2D1_DRAW_TEXT_OPTIONS_NONE);
                canvas.SetTransform(&matrix(1.0, 0.0, 0.0));
            },
        )?;
        let sz = image.GetSize();
        self.tint(
            rt,
            &image,
            rect(left, top, left + sz.width, top + sz.height),
            c,
        )
    }
    unsafe fn mask(
        &mut self,
        rt: &ID2D1RenderTarget,
        key: String,
        w: f32,
        h: f32,
        dpi: f32,
        draw: impl FnOnce(&ID2D1BitmapRenderTarget, &ID2D1SolidColorBrush),
    ) -> Result<ID2D1Bitmap> {
        self.serial += 1;
        if let Some(m) = self.masks.get_mut(&key) {
            m.used = self.serial;
            return Ok(m.image.clone());
        }
        if self.masks.len() >= 256 {
            if let Some(k) = self
                .masks
                .iter()
                .min_by_key(|(_, m)| m.used)
                .map(|(k, _)| k.clone())
            {
                self.masks.remove(&k);
            }
        }
        let size = D2D_SIZE_U {
            width: (w * dpi / 96.0).ceil().max(1.0) as u32,
            height: (h * dpi / 96.0).ceil().max(1.0) as u32,
        };
        let canvas = rt.CreateCompatibleRenderTarget(
            None,
            Some(&size),
            Some(&D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            }),
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )?;
        canvas.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        let white = canvas.CreateSolidColorBrush(&color(0xffffff, 1.0), None)?;
        canvas.BeginDraw();
        canvas.Clear(Some(&color(0, 0.0)));
        draw(&canvas, &white);
        canvas.EndDraw(None, None)?;
        let image = canvas.GetBitmap()?;
        self.masks.insert(
            key,
            Mask {
                image: image.clone(),
                used: self.serial,
            },
        );
        self.mask_builds += 1;
        Ok(image)
    }
    unsafe fn tint(
        &mut self,
        rt: &ID2D1RenderTarget,
        image: &ID2D1Bitmap,
        b: Rect,
        c: Color,
    ) -> Result<()> {
        let brush = self.ink(rt, c)?;
        let previous = rt.GetAntialiasMode();
        rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
        rt.FillOpacityMask(
            image,
            &brush,
            D2D1_OPACITY_MASK_CONTENT_GRAPHICS,
            Some(&b),
            None,
        );
        rt.SetAntialiasMode(previous);
        Ok(())
    }
    unsafe fn draw_static(&mut self, rt: &ID2D1RenderTarget, g: Geometry, dpi: f32) -> Result<()> {
        if self.paper.is_none() {
            let stops = [
                D2D1_GRADIENT_STOP {
                    position: 0.0,
                    color: color(0xeccd9e, 1.0),
                },
                D2D1_GRADIENT_STOP {
                    position: 1.0,
                    color: color(0xcfae7c, 1.0),
                },
            ];
            let collection =
                rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
            self.paper = Some(rt.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: point(0.0, 0.0),
                    endPoint: point(g.width * 0.35, g.height),
                },
                None,
                &collection,
            )?);
        }
        rt.FillRectangle(&g.full(), self.paper.as_ref().unwrap());
        let b = self.ink(rt, color(0x785b3b, 0.4))?;
        rt.DrawRectangle(
            &rect(14.0, 14.0, g.width - 14.0, g.height - 14.0),
            &b,
            1.0,
            None,
        );
        for x in [15.0, g.width - 15.0] {
            for y in [15.0, g.height - 15.0] {
                rt.FillEllipse(&ellipse(x, y, 6.5), &self.ink(rt, color(0x555b52, 1.0))?);
                self.line(
                    rt,
                    x - 3.0,
                    y + 3.0,
                    x + 3.0,
                    y - 3.0,
                    color(0x171c1a, 1.0),
                    1.0,
                )?;
            }
        }
        let header_bottom = g.ruler_y - (g.height * 0.017).clamp(6.0, 18.0);
        self.text(
            rt,
            "ENGINEERING",
            g.heading,
            rect(g.margin, g.top, g.right, header_bottom - 6.0),
            color(0x47301e, 1.0),
            0,
            dpi,
            false,
        )?;
        self.line(
            rt,
            g.margin,
            header_bottom,
            g.right,
            header_bottom,
            color(0x806443, 0.5),
            1.0,
        )?;
        let left = g.slider_left + g.grip_size / 2.0;
        let right = g.slider_right - g.grip_size / 2.0;
        for (i, label) in LABELS.iter().enumerate() {
            let top = g.body_top + i as f32 * g.row;
            let y = top + g.row / 2.0;
            self.text(
                rt,
                label,
                (g.width * 0.033).min(g.height * 0.058).clamp(18.0, 58.0),
                rect(g.margin, top, g.slider_left - 12.0, top + g.row),
                color(0x47301e, 1.0),
                0,
                dpi,
                false,
            )?;
            let tick = 22.0f32.min(g.row / 2.0 - 2.0);
            for j in 0..4 {
                let x = left + (right - left) * j as f32 / 3.0;
                self.line(rt, x, y - tick, x, y + tick, color(0x6f5638, 0.25), 1.0)?;
            }
        }
        self.line(
            rt,
            g.margin,
            g.bottom + 5.0,
            g.right,
            g.bottom + 5.0,
            color(0x806443, 0.44),
            1.0,
        )?;
        let (_, _, _, _, _, housing) = g.footer();
        rt.FillRoundedRectangle(&rounded(housing, 9.0), &self.ink(rt, color(0x353b32, 1.0))?);
        let inner = rect(
            housing.left + 7.0,
            housing.top + 7.0,
            housing.right - 7.0,
            housing.bottom - 7.0,
        );
        rt.FillRoundedRectangle(&rounded(inner, 4.0), &self.ink(rt, color(0x264133, 1.0))?);
        Ok(())
    }
    pub unsafe fn backdrop(&mut self, rt: &ID2D1RenderTarget, g: Geometry, dpi: f32) -> Result<()> {
        let owner = rt.as_raw() as usize;
        if owner != self.owner {
            self.clear();
            self.owner = owner;
        }
        if self.backdrop.is_some() {
            return Ok(());
        }
        let sz = D2D_SIZE_U {
            width: (g.width * dpi / 96.0).ceil() as u32,
            height: (g.height * dpi / 96.0).ceil() as u32,
        };
        let canvas = rt.CreateCompatibleRenderTarget(
            None,
            Some(&sz),
            Some(&D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            }),
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )?;
        canvas.BeginDraw();
        self.draw_static(&canvas, g, dpi)?;
        canvas.EndDraw(None, None)?;
        self.backdrop = Some(canvas.GetBitmap()?);
        self.background_builds += 1;
        Ok(())
    }
    unsafe fn row(
        &mut self,
        rt: &ID2D1RenderTarget,
        bitmap: &ID2D1Bitmap,
        g: Geometry,
        i: usize,
        v: &Visual,
        dpi: f32,
    ) -> Result<()> {
        let top = g.body_top + i as f32 * g.row;
        let y = top + g.row / 2.0;
        let left = g.slider_left + g.grip_size / 2.0;
        let right = g.slider_right - g.grip_size / 2.0;
        self.fill(rt, rect(left, y - 6.5, right, y + 6.5), v.rail)?;
        let handle = left + (v.positions[i] / 300.0) as f32 * (right - left);
        self.fill(
            rt,
            rect(
                left,
                y - 6.5,
                left + (v.positions[i].clamp(0.0, 300.0) / 300.0) as f32 * (right - left),
                y + 6.5,
            ),
            v.accent,
        )?;
        self.line(
            rt,
            left,
            y - 6.5,
            right,
            y - 6.5,
            color(0x200f0a, 0.33),
            2.0,
        )?;
        rt.DrawBitmap(
            bitmap,
            Some(&rect(
                handle - g.grip_size / 2.0,
                y - g.grip_size / 2.0,
                handle + g.grip_size / 2.0,
                y + g.grip_size / 2.0,
            )),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
        self.text(
            rt,
            &v.readouts[i],
            (g.height * 0.043).clamp(18.0, 42.0),
            rect(g.slider_right + 4.0, top, g.right, top + g.row),
            v.accent,
            2,
            dpi,
            true,
        )?;
        if v.focus == i as i32 {
            rt.DrawRoundedRectangle(
                &rounded(
                    rect(g.slider_left, top + 2.0, g.slider_right, top + g.row - 2.0),
                    4.0,
                ),
                &self.ink(rt, color(0x47301e, 0.8))?,
                1.0,
                None,
            );
        }
        Ok(())
    }
    unsafe fn footer(
        &mut self,
        rt: &ID2D1RenderTarget,
        g: Geometry,
        v: &Visual,
        dpi: f32,
    ) -> Result<()> {
        let (d, cy, _, ml, size, housing) = g.footer();
        for layer in 0..3 {
            let image = self.mask(
                rt,
                format!("reset|{d:.6}|{layer}"),
                d + 4.0,
                d + 4.0,
                dpi,
                |canvas, white| {
                    let mut circle = ellipse(2.0 + d / 2.0, 2.0 + d / 2.0, d / 2.0 - 3.0);
                    if layer == 0 {
                        canvas.FillEllipse(&circle, white);
                    } else if layer == 1 {
                        canvas.DrawEllipse(&circle, white, 6.0, None);
                    } else {
                        circle.radiusX -= 6.0;
                        circle.radiusY -= 6.0;
                        canvas.DrawEllipse(&circle, white, 2.0, None);
                    }
                },
            )?;
            let sz = image.GetSize();
            self.tint(
                rt,
                &image,
                rect(
                    g.margin - 2.0,
                    cy - d / 2.0 - 2.0,
                    g.margin - 2.0 + sz.width,
                    cy - d / 2.0 - 2.0 + sz.height,
                ),
                if layer == 0 {
                    v.accent
                } else if layer == 1 {
                    v.rail
                } else {
                    color(0xffffff, 0.53)
                },
            )?;
        }
        self.text(
            rt,
            "RESET\nLEVELS",
            (g.height * 0.038).clamp(17.0, 38.0),
            rect(
                g.margin + d + 12.0,
                g.reset.top,
                g.reset.right,
                g.reset.bottom,
            ),
            v.accent,
            0,
            dpi,
            true,
        )?;
        let muted = color(0x876749, 1.0);
        self.text(
            rt,
            "POWER",
            size,
            rect(ml, g.toggle.top, g.toggle.left - 10.0, g.toggle.bottom),
            if v.cooling { muted } else { v.accent },
            2,
            dpi,
            true,
        )?;
        self.text(
            rt,
            "COOLANT",
            size,
            rect(
                g.toggle.right + 10.0,
                g.toggle.top,
                g.right,
                g.toggle.bottom,
            ),
            if v.cooling { v.accent } else { muted },
            0,
            dpi,
            true,
        )?;
        let inner = rect(
            housing.left + 7.0,
            housing.top + 7.0,
            housing.right - 7.0,
            housing.bottom - 7.0,
        );
        let lever = (inner.right - inner.left) * 0.4;
        let x = inner.left + 3.0 + v.lever as f32 * (inner.right - inner.left - lever - 6.0);
        rt.FillRoundedRectangle(
            &rounded(rect(x, inner.top + 3.0, x + lever, inner.bottom - 3.0), 2.0),
            &self.ink(rt, color(0xd1d6b6, 1.0))?,
        );
        if v.focus >= 8 {
            rt.DrawRoundedRectangle(
                &rounded(if v.focus == 8 { g.reset } else { g.toggle }, 5.0),
                &self.ink(rt, color(0x47301e, 1.0))?,
                2.0,
                None,
            );
        }
        Ok(())
    }
    pub unsafe fn region(
        &mut self,
        rt: &ID2D1RenderTarget,
        bitmap: &ID2D1Bitmap,
        g: Geometry,
        dpi: f32,
        r: Rect,
        v: &Visual,
    ) -> Result<()> {
        rt.PushAxisAlignedClip(&r, D2D1_ANTIALIAS_MODE_ALIASED);
        let result = (|| -> Result<()> {
            rt.DrawBitmap(
                self.backdrop.as_ref().unwrap(),
                Some(&r),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                Some(&r),
            );
            for i in 0..8 {
                if intersects(r, g.row_bounds(i)) {
                    self.row(rt, bitmap, g, i, v, dpi)?;
                }
            }
            if intersects(r, g.ruler_bounds()) {
                let left = g.slider_left + g.grip_size / 2.0;
                let right = g.slider_right - g.grip_size / 2.0;
                let size = (g.width * 0.014).min(g.height * 0.027).clamp(10.0, 25.0);
                for i in 0..4 {
                    let x = left + (right - left) * i as f32 / 3.0;
                    self.text(
                        rt,
                        &v.ruler[i],
                        size,
                        rect(
                            if i == 0 { x } else { x - 48.0 },
                            g.ruler_y,
                            if i == 3 { x } else { x + 48.0 },
                            g.body_top - 4.0,
                        ),
                        color(0x735238, 1.0),
                        if i == 0 {
                            0
                        } else if i == 3 {
                            2
                        } else {
                            1
                        },
                        dpi,
                        true,
                    )?;
                }
            }
            if intersects(r, g.footer_bounds()) {
                self.footer(rt, g, v, dpi)?;
            }
            if !v.status.is_empty() && intersects(r, g.status_bounds()) {
                self.text(
                    rt,
                    &v.status,
                    11.0,
                    rect(g.right - 190.0, g.top, g.right, g.top + 16.0),
                    color(0x876749, 1.0),
                    2,
                    dpi,
                    true,
                )?;
            }
            Ok(())
        })();
        rt.PopAxisAlignedClip();
        result
    }
    pub fn window_resources(&self) -> (ID2D1RenderTarget, ID2D1Bitmap) {
        (
            self.target.as_ref().unwrap().cast().unwrap(),
            self.grip.as_ref().unwrap().clone(),
        )
    }
    pub unsafe fn offscreen(
        &self,
        g: Geometry,
        dpi: f32,
    ) -> Result<(IWICBitmap, ID2D1RenderTarget, ID2D1Bitmap)> {
        let bitmap = self.imaging.CreateBitmap(
            (g.width * dpi / 96.0).round() as u32,
            (g.height * dpi / 96.0).round() as u32,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?;
        let rt = self
            .factory
            .CreateWicBitmapRenderTarget(&bitmap, &Self::properties(dpi))?;
        let grip = rt.CreateBitmapFromWicBitmap(&self.pixels, None)?;
        Ok((bitmap, rt, grip))
    }
    pub unsafe fn save_png(&self, path: &Path, bitmap: &IWICBitmap) -> Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let stream = self.imaging.CreateStream()?;
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        stream.InitializeFromFilename(PCWSTR(name.as_ptr()), GENERIC_WRITE.0)?;
        let encoder = self
            .imaging
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
        let frame = frame.unwrap();
        frame.Initialize(None)?;
        let (mut w, mut h) = (0, 0);
        bitmap.GetSize(&mut w, &mut h)?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format)?;
        frame.WriteSource(bitmap, std::ptr::null())?;
        frame.Commit()?;
        encoder.Commit()?;
        Ok(())
    }
}
use std::os::windows::ffi::OsStrExt;
impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .text_factory
                .UnregisterFontFileLoader(&self.font_loader);
        }
    }
}
