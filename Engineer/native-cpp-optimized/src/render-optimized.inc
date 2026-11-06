// Included inside Panel. All caches are device/DPI dependent and local to this
// version. The controller, motion curve, artwork and transport are unchanged.
struct CachedMask { ComPtr<ID2D1Bitmap> image; uint64_t used = 0; };
std::map<std::wstring, CachedMask> masks;
ComPtr<ID2D1Bitmap> backdrop;
ID2D1RenderTarget* cacheOwner = nullptr;
std::unique_ptr<FrameClock> frameClock;
bool needsFull = true;
uint64_t maskSerial = 0, maskBuilds = 0, backgroundBuilds = 0, fullDraws = 0, partialDraws = 0;
double lastDrawArea = 0;
struct Visual {
    std::array<double, 8> positions{};
    std::array<std::wstring, 8> readouts;
    std::array<std::wstring, 4> ruler;
    D2D1_COLOR_F accent{}, rail{};
    bool cooling = false; double lever = 0; int focused = -1;
    std::wstring status;
};
std::optional<Visual> lastVisual;

void clearRenderCaches() {
    masks.clear(); backdrop.Reset(); paper.Reset(); brush.Reset(); cacheOwner = nullptr;
    lastVisual.reset(); needsFull = true;
}
static bool sameColor(D2D1_COLOR_F a, D2D1_COLOR_F b) {
    return a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a;
}
static bool intersects(D2D1_RECT_F a, D2D1_RECT_F b) {
    return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
}
Visual visual(double t) {
    Visual v; v.accent = accent(t); v.rail = rail(t); v.cooling = state.cooling; v.lever = rocker(t);
    v.focused = showFocus ? focus : -1;
    v.status = options.profile.empty() && sender ? wide(sender->getStatus()) : L"";
    auto a = allocate(state.coolant);
    for (int i = 0; i < 8; ++i) { v.positions[i] = motion[i].sample(t); v.readouts[i] = percent(state.cooling ? a.percentages[i] : state.power[i]); }
    for (int i = 0; i < 4; ++i) v.ruler[i] = percent((state.cooling ? a.maximum : 300) * i / 3);
    return v;
}
D2D1_RECT_F rowBounds(int i) const {
    // Include the entire maximum decorative overshoot, the grip and AA fringe.
    float overhang = std::max(0.f, g.sliderRight - g.sliderLeft - g.gripSize) * .11f + 3;
    return D2D1::RectF(g.sliderLeft - overhang, g.bodyTop + i * g.row - 2, g.right + 3, g.bodyTop + (i + 1) * g.row + 2);
}
D2D1_RECT_F rulerBounds() const { return D2D1::RectF(g.sliderLeft - 3, g.rulerY - 3, g.sliderRight + 3, g.bodyTop + 1); }
D2D1_RECT_F footerBounds() const { return D2D1::RectF(g.margin - 3, g.reset.top - 3, g.right + 3, g.reset.bottom + 3); }
D2D1_RECT_F statusBounds() const { return D2D1::RectF(g.right - 193, g.top - 2, g.right + 2, g.top + 19); }
std::vector<D2D1_RECT_F> changedRegions(const Visual& v) const {
    if (needsFull || !lastVisual) return {D2D1::RectF(0, 0, width, height)};
    const auto& old = *lastVisual; std::vector<D2D1_RECT_F> dirty;
    bool colors = !sameColor(v.accent, old.accent) || !sameColor(v.rail, old.rail);
    for (int i = 0; i < 8; ++i) {
        bool focusChanged = (v.focused == i) != (old.focused == i);
        if (colors || v.positions[i] != old.positions[i] || v.readouts[i] != old.readouts[i] || focusChanged) dirty.push_back(rowBounds(i));
    }
    if (v.ruler != old.ruler) dirty.push_back(rulerBounds());
    if (colors || v.cooling != old.cooling || v.lever != old.lever ||
        ((v.focused >= 8 || old.focused >= 8) && v.focused != old.focused)) dirty.push_back(footerBounds());
    if (v.status != old.status) dirty.push_back(statusBounds());
    return dirty;
}
RECT physical(D2D1_RECT_F r) const {
    return {static_cast<LONG>(std::floor(std::max(0.f, r.left) * dpi / 96)),
        static_cast<LONG>(std::floor(std::max(0.f, r.top) * dpi / 96)),
        static_cast<LONG>(std::ceil(std::min(width, r.right) * dpi / 96)),
        static_cast<LONG>(std::ceil(std::min(height, r.bottom) * dpi / 96))};
}
std::vector<RECT> updateRectangles() const {
    HRGN region = CreateRectRgn(0, 0, 0, 0); std::vector<RECT> result;
    if (GetUpdateRgn(hwnd, region, FALSE) != ERROR) {
        DWORD size = GetRegionData(region, 0, nullptr);
        if (size) { std::vector<BYTE> bytes(size); auto data = reinterpret_cast<RGNDATA*>(bytes.data());
            if (GetRegionData(region, size, data)) { auto rectangles = reinterpret_cast<const RECT*>(data->Buffer); result.assign(rectangles, rectangles + data->rdh.nCount); }
        }
    }
    DeleteObject(region); return result;
}
void requestFrame() {
    if (!hwnd) return;
    if (!frameClock) { InvalidateRect(hwnd, nullptr, FALSE); return; }
    double t = nowMs(); bool animate = moving(t);
    frameClock->active(!IsIconic(hwnd) && (animate || !changedRegions(visual(t)).empty()));
}
void frameReady() {
    frameClock->consumed(); if (IsIconic(hwnd)) { frameClock->active(false); return; }
    auto dirty = changedRegions(visual(nowMs()));
    for (auto r : dirty) { auto pixelsRect = physical(r); InvalidateRect(hwnd, &pixelsRect, FALSE); }
    if (dirty.empty()) frameClock->active(moving(nowMs()));
}

template<class Draw>
ComPtr<ID2D1Bitmap> mask(ID2D1RenderTarget* rt, const std::wstring& key, D2D1_SIZE_F size, Draw render) {
    auto found = masks.find(key);
    if (found != masks.end()) { found->second.used = ++maskSerial; return found->second.image; }
    // Bound the cache for long dragging sessions with many different readouts.
    if (masks.size() >= 256) {
        auto oldest = std::min_element(masks.begin(), masks.end(), [](const auto& a, const auto& b) { return a.second.used < b.second.used; });
        masks.erase(oldest);
    }
    auto pixelsSize = D2D1::SizeU(std::max(1u, static_cast<UINT32>(std::ceil(size.width * dpi / 96))), std::max(1u, static_cast<UINT32>(std::ceil(size.height * dpi / 96))));
    auto format = D2D1::PixelFormat(DXGI_FORMAT_A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED);
    ComPtr<ID2D1BitmapRenderTarget> canvas;
    check(rt->CreateCompatibleRenderTarget(nullptr, &pixelsSize, &format, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, &canvas));
    canvas->SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
    ComPtr<ID2D1SolidColorBrush> white; check(canvas->CreateSolidColorBrush(D2D1::ColorF(1.f, 1.f, 1.f, 1.f), &white));
    canvas->BeginDraw(); canvas->Clear(D2D1::ColorF(0, 0.f)); render(canvas.Get(), white.Get()); check(canvas->EndDraw());
    ComPtr<ID2D1Bitmap> image; check(canvas->GetBitmap(&image));
    masks.emplace(key, CachedMask{image, ++maskSerial}); ++maskBuilds; return image;
}
void tint(ID2D1RenderTarget* rt, ID2D1Bitmap* image, D2D1_RECT_F bounds, D2D1_COLOR_F c) {
    ink(rt, c); auto previous = rt->GetAntialiasMode(); rt->SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
    rt->FillOpacityMask(image, brush.Get(), D2D1_OPACITY_MASK_CONTENT_GRAPHICS, &bounds); rt->SetAntialiasMode(previous);
}
void text(ID2D1RenderTarget* rt, const std::wstring& s, float size, D2D1_RECT_F box, D2D1_COLOR_F c, int align = 0) {
    std::wstring layoutKey = s + L"\x1f" + std::to_wstring(static_cast<int>(size * 100));
    ComPtr<IDWriteTextLayout> layout;
    auto found = layouts.find(layoutKey);
    if (found != layouts.end()) layout = found->second;
    else {
        ComPtr<IDWriteTextFormat> format; check(textFactory->CreateTextFormat(L"Russo One", fonts.Get(), DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, L"en-US", &format));
        check(format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)); check(textFactory->CreateTextLayout(s.c_str(), static_cast<UINT32>(s.size()), format.Get(), 4096, 512, &layout));
        if (layouts.size() > 512) layouts.clear(); layouts.emplace(layoutKey, layout);
    }
    DWRITE_TEXT_METRICS metrics{}; check(layout->GetMetrics(&metrics));
    float scale = std::min({1.f, (box.right - box.left) / std::max(1.f, metrics.widthIncludingTrailingWhitespace), (box.bottom - box.top) / std::max(1.f, metrics.height)});
    float x = box.left + (align == 2 ? box.right - box.left - metrics.widthIncludingTrailingWhitespace * scale : align == 1 ? (box.right - box.left - metrics.widthIncludingTrailingWhitespace * scale) / 2 : 0);
    float y = box.top + (box.bottom - box.top - metrics.height * scale) / 2;
    float left = std::floor(x * dpi / 96) * 96 / dpi - 3, top = std::floor(y * dpi / 96) * 96 / dpi - 3;
    D2D1_SIZE_F dimensions = D2D1::SizeF(metrics.widthIncludingTrailingWhitespace * scale + 8, metrics.height * scale + 8);
    auto key = L"text|" + layoutKey + L"|" + std::to_wstring(scale) + L"|" + std::to_wstring(x - left) + L"|" + std::to_wstring(y - top);
    auto image = mask(rt, key, dimensions, [&](ID2D1RenderTarget* canvas, ID2D1Brush* white) {
        canvas->SetTransform(D2D1::Matrix3x2F::Scale(scale, scale) * D2D1::Matrix3x2F::Translation(x - left, y - top));
        canvas->DrawTextLayout(D2D1::Point2F(0, 0), layout.Get(), white); canvas->SetTransform(D2D1::Matrix3x2F::Identity());
    });
    auto imageSize = image->GetSize(); tint(rt, image.Get(), D2D1::RectF(left, top, left + imageSize.width, top + imageSize.height), c);
}
struct FooterGeometry { float diameter, cy, modeWidth, modeLeft, modeSize; D2D1_RECT_F housing; };
FooterGeometry footerGeometry() const {
    FooterGeometry f{}; f.diameter = std::min(std::clamp(height * .081f, 44.f, 78.f), g.reset.bottom - g.reset.top); f.cy = (g.reset.top + g.reset.bottom) / 2;
    f.modeSize = std::clamp(std::min(width * .026f, height * .048f), 18.f, 42.f);
    f.modeWidth = std::min((width - 2 * g.margin) * .62f, f.modeSize * 12.5f); f.modeLeft = g.right - f.modeWidth;
    float toggleW = std::min(82.f, g.toggle.right - g.toggle.left), toggleH = std::min(58.f, g.toggle.bottom - g.toggle.top), cx = (g.toggle.left + g.toggle.right) / 2;
    f.housing = D2D1::RectF(cx - toggleW / 2, f.cy - toggleH / 2, cx + toggleW / 2, f.cy + toggleH / 2); return f;
}
void drawStatic(ID2D1RenderTarget* rt) {
    auto dark = color(0x47301e); ensurePaper(rt); rt->FillRectangle(D2D1::RectF(0, 0, width, height), paper.Get());
    ink(rt, color(0x785b3b, .4f)); rt->DrawRectangle(D2D1::RectF(14, 14, width - 14, height - 14), brush.Get());
    for (float x : {15.f, width - 15}) for (float y : {15.f, height - 15}) { ink(rt, color(0x555b52)); rt->FillEllipse(D2D1::Ellipse(D2D1::Point2F(x, y), 6.5f, 6.5f), brush.Get()); line(rt, x - 3, y + 3, x + 3, y - 3, color(0x171c1a)); }
    float headerBottom = g.rulerY - std::clamp(height * .017f, 6.f, 18.f);
    textDirect(rt, L"ENGINEERING", g.headingSize, D2D1::RectF(g.margin, g.top, g.right, headerBottom - 6), dark);
    line(rt, g.margin, headerBottom, g.right, headerBottom, color(0x806443, .5f));
    float left = g.sliderLeft + g.gripSize / 2, right = g.sliderRight - g.gripSize / 2;
    for (int i = 0; i < 8; ++i) {
        float top = g.bodyTop + i * g.row, y = top + g.row / 2;
        textDirect(rt, labels[i], std::clamp(std::min(width * .033f, height * .058f), 18.f, 58.f), D2D1::RectF(g.margin, top, g.sliderLeft - 12, top + g.row), dark);
        float tick = std::min(22.f, g.row / 2 - 2);
        for (int j = 0; j < 4; ++j) { float x = left + (right - left) * j / 3; line(rt, x, y - tick, x, y + tick, color(0x6f5638, .25f)); }
    }
    line(rt, g.margin, g.bottom + 5, g.right, g.bottom + 5, color(0x806443, .44f));
    auto f = footerGeometry(); ink(rt, color(0x353b32)); rt->FillRoundedRectangle(D2D1::RoundedRect(f.housing, 9, 9), brush.Get());
    auto inner = f.housing; inner.left += 7; inner.right -= 7; inner.top += 7; inner.bottom -= 7;
    ink(rt, color(0x264133)); rt->FillRoundedRectangle(D2D1::RoundedRect(inner, 4, 4), brush.Get());
}
void ensureBackdrop(ID2D1RenderTarget* rt) {
    if (cacheOwner != rt) { clearRenderCaches(); cacheOwner = rt; }
    if (backdrop) return;
    auto pixelsSize = D2D1::SizeU(static_cast<UINT32>(std::ceil(width * dpi / 96)), static_cast<UINT32>(std::ceil(height * dpi / 96)));
    auto format = D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE);
    ComPtr<ID2D1BitmapRenderTarget> canvas; check(rt->CreateCompatibleRenderTarget(nullptr, &pixelsSize, &format, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, &canvas));
    canvas->BeginDraw(); drawStatic(canvas.Get()); check(canvas->EndDraw()); check(canvas->GetBitmap(&backdrop)); ++backgroundBuilds;
}
void drawRow(ID2D1RenderTarget* rt, ID2D1Bitmap* bitmap, int i, const Visual& v) {
    float top = g.bodyTop + i * g.row, y = top + g.row / 2, left = g.sliderLeft + g.gripSize / 2, right = g.sliderRight - g.gripSize / 2;
    rect(rt, D2D1::RectF(left, y - 6.5f, right, y + 6.5f), v.rail);
    float handle = left + static_cast<float>(v.positions[i] / 300) * (right - left);
    rect(rt, D2D1::RectF(left, y - 6.5f, left + static_cast<float>(std::clamp(v.positions[i], 0.0, 300.0) / 300) * (right - left), y + 6.5f), v.accent);
    line(rt, left, y - 6.5f, right, y - 6.5f, color(0x200f0a, .33f), 2);
    rt->DrawBitmap(bitmap, D2D1::RectF(handle - g.gripSize / 2, y - g.gripSize / 2, handle + g.gripSize / 2, y + g.gripSize / 2), 1, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    text(rt, v.readouts[i], std::clamp(height * .043f, 18.f, 42.f), D2D1::RectF(g.readoutLeft + 4, top, g.right, top + g.row), v.accent, 2);
    if (v.focused == i) { ink(rt, color(0x47301e, .8f)); rt->DrawRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(g.sliderLeft, top + 2, g.sliderRight, top + g.row - 2), 4, 4), brush.Get(), 1); }
}
void drawFooter(ID2D1RenderTarget* rt, const Visual& v) {
    auto f = footerGeometry(); float d = f.diameter;
    for (int layer = 0; layer < 3; ++layer) {
        auto image = mask(rt, L"reset|" + std::to_wstring(d) + L"|" + std::to_wstring(layer), D2D1::SizeF(d + 4, d + 4), [=](ID2D1RenderTarget* canvas, ID2D1Brush* white) {
            auto circle = D2D1::Ellipse(D2D1::Point2F(2 + d / 2, 2 + d / 2), d / 2 - 3, d / 2 - 3);
            if (!layer) canvas->FillEllipse(circle, white);
            else if (layer == 1) canvas->DrawEllipse(circle, white, 6);
            else { circle.radiusX -= 6; circle.radiusY -= 6; canvas->DrawEllipse(circle, white, 2); }
        });
        auto size = image->GetSize(); tint(rt, image.Get(), D2D1::RectF(g.margin - 2, f.cy - d / 2 - 2, g.margin - 2 + size.width, f.cy - d / 2 - 2 + size.height), layer == 0 ? v.accent : layer == 1 ? v.rail : color(0xffffff, .53f));
    }
    text(rt, L"RESET\nLEVELS", std::clamp(height * .038f, 17.f, 38.f), D2D1::RectF(g.margin + d + 12, g.reset.top, g.reset.right, g.reset.bottom), v.accent);
    auto muted = color(0x876749);
    text(rt, L"POWER", f.modeSize, D2D1::RectF(f.modeLeft, g.toggle.top, g.toggle.left - 10, g.toggle.bottom), v.cooling ? muted : v.accent, 2);
    text(rt, L"COOLANT", f.modeSize, D2D1::RectF(g.toggle.right + 10, g.toggle.top, g.right, g.toggle.bottom), v.cooling ? v.accent : muted);
    auto inner = f.housing; inner.left += 7; inner.right -= 7; inner.top += 7; inner.bottom -= 7;
    float lever = (inner.right - inner.left) * .4f, x = inner.left + 3 + static_cast<float>(v.lever) * (inner.right - inner.left - lever - 6);
    ink(rt, color(0xd1d6b6)); rt->FillRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(x, inner.top + 3, x + lever, inner.bottom - 3), 2, 2), brush.Get());
    if (v.focused >= 8) { ink(rt, color(0x47301e)); rt->DrawRoundedRectangle(D2D1::RoundedRect(v.focused == 8 ? g.reset : g.toggle, 5, 5), brush.Get(), 2); }
}
void drawRuler(ID2D1RenderTarget* rt, const Visual& v) {
    float left = g.sliderLeft + g.gripSize / 2, right = g.sliderRight - g.gripSize / 2;
    float size = std::clamp(std::min(width * .014f, height * .027f), 10.f, 25.f);
    for (int i = 0; i < 4; ++i) { float x = left + (right - left) * i / 3; auto r = D2D1::RectF(x - 48, g.rulerY, x + 48, g.bodyTop - 4); if (!i) r.left = x; if (i == 3) r.right = x; text(rt, v.ruler[i], size, r, color(0x735238), !i ? 0 : i == 3 ? 2 : 1); }
}
void drawRegion(ID2D1RenderTarget* rt, ID2D1Bitmap* bitmap, D2D1_RECT_F r, const Visual& v) {
    rt->PushAxisAlignedClip(r, D2D1_ANTIALIAS_MODE_ALIASED);
    // Restore old grips, fills, readouts and alpha edges before drawing new ones.
    rt->DrawBitmap(backdrop.Get(), r, 1, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, r);
    for (int i = 0; i < 8; ++i) if (intersects(r, rowBounds(i))) drawRow(rt, bitmap, i, v);
    if (intersects(r, rulerBounds())) drawRuler(rt, v);
    if (intersects(r, footerBounds())) drawFooter(rt, v);
    if (!v.status.empty() && intersects(r, statusBounds())) text(rt, v.status, 11, D2D1::RectF(g.right - 190, g.top, g.right, g.top + 16), color(0x876749), 2);
    rt->PopAxisAlignedClip();
}
void draw(ID2D1RenderTarget* rt, ID2D1Bitmap* bitmap) {
    ensureBackdrop(rt); drawRegion(rt, bitmap, D2D1::RectF(0, 0, width, height), visual(nowMs()));
}
void paint(const std::vector<RECT>& exposed = {}) {
    if (IsIconic(hwnd)) return;
    deviceResources(); if (!target) return; ensureBackdrop(target.Get());
    double t = nowMs(); auto v = visual(t); auto dirty = changedRegions(v);
    bool full = needsFull || !lastVisual;
    HRGN region = CreateRectRgn(0, 0, 0, 0);
    auto add = [&](RECT r) { HRGN part = CreateRectRgnIndirect(&r); CombineRgn(region, region, part, RGN_OR); DeleteObject(part); };
    for (auto r : dirty) add(physical(r));
    for (auto r : exposed) add(r);
    DWORD size = GetRegionData(region, 0, nullptr); std::vector<BYTE> bytes(size);
    if (!size || !GetRegionData(region, size, reinterpret_cast<RGNDATA*>(bytes.data()))) { DeleteObject(region); requestFrame(); return; }
    DeleteObject(region); auto data = reinterpret_cast<RGNDATA*>(bytes.data()); auto rectangles = reinterpret_cast<RECT*>(data->Buffer);
    if (!data->rdh.nCount) { requestFrame(); return; }
    target->BeginDraw(); lastDrawArea = 0;
    for (DWORD i = 0; i < data->rdh.nCount; ++i) {
        const auto& r = rectangles[i]; lastDrawArea += static_cast<double>(r.right - r.left) * (r.bottom - r.top);
        drawRegion(target.Get(), grip.Get(), D2D1::RectF(r.left * 96.f / dpi, r.top * 96.f / dpi, r.right * 96.f / dpi, r.bottom * 96.f / dpi), v);
    }
    auto result = target->EndDraw(); ++draws; if (full) ++fullDraws; else ++partialDraws;
    if (phase >= 0 && previousDraw != 0) drawIntervals.push_back(t - previousDraw); previousDraw = t;
    if (result == D2DERR_RECREATE_TARGET) { discard(); requestFrame(); return; }
    check(result); lastVisual = std::move(v); needsFull = false; requestFrame();
}
