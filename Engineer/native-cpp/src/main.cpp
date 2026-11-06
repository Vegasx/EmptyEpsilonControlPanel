#ifndef UNICODE
#define UNICODE
#endif
#define _UNICODE
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <windowsx.h>
#include <d2d1.h>
#include <dwrite_3.h>
#include <wincodec.h>
#include <wrl/client.h>
#include <shellapi.h>
#include <psapi.h>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <memory>
#include <sstream>
#include "transport.h"

using Microsoft::WRL::ComPtr;
using namespace epsilon;
namespace fs = std::filesystem;
constexpr UINT NetworkChanged = WM_APP + 1;
static void check(HRESULT hr) { if (FAILED(hr)) throw std::runtime_error("Windows graphics error " + std::to_string(static_cast<unsigned long>(hr))); }
static double nowMs() { static LARGE_INTEGER frequency = [] { LARGE_INTEGER f; QueryPerformanceFrequency(&f); return f; }(); LARGE_INTEGER t; QueryPerformanceCounter(&t); return t.QuadPart * 1000.0 / frequency.QuadPart; }
static double cpuMs() { FILETIME c, e, k, u; GetProcessTimes(GetCurrentProcess(), &c, &e, &k, &u); ULARGE_INTEGER a, b; a.LowPart = k.dwLowDateTime; a.HighPart = k.dwHighDateTime; b.LowPart = u.dwLowDateTime; b.HighPart = u.dwHighDateTime; return (a.QuadPart + b.QuadPart) / 10000.0; }
static std::string utcNow() { SYSTEMTIME t{}; GetSystemTime(&t); char text[40]; std::snprintf(text, sizeof(text), "%04u-%02u-%02uT%02u:%02u:%02u.%03uZ", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds); return text; }
static std::string readFile(const fs::path& path) { std::ifstream f(path, std::ios::binary); if (!f) throw std::runtime_error("Cannot open " + narrow(path.wstring())); f.seekg(0, std::ios::end); auto n = f.tellg(); if (n < 0 || n > 1024 * 1024) throw std::runtime_error("Configuration exceeds 1 MiB"); f.seekg(0); std::string s(static_cast<size_t>(n), 0); if (!s.empty() && !f.read(s.data(), n)) throw std::runtime_error("Cannot read configuration"); return s; }
static void writeFile(const fs::path& path, const std::string& s) { if (!path.parent_path().empty()) fs::create_directories(path.parent_path()); std::ofstream f(path, std::ios::binary); if (!f || !(f << s)) throw std::runtime_error("Cannot write " + narrow(path.wstring())); }
static std::pair<const void*, DWORD> resource(int id) { auto r = FindResourceW(nullptr, MAKEINTRESOURCEW(id), RT_RCDATA); if (!r) throw std::runtime_error("Embedded asset missing"); auto loaded = LoadResource(nullptr, r); return {LockResource(loaded), SizeofResource(nullptr, r)}; }
static D2D1_COLOR_F color(unsigned rgb, float alpha = 1) { return D2D1::ColorF(rgb, alpha); }
static D2D1_COLOR_F mix(D2D1_COLOR_F a, D2D1_COLOR_F b, double t) { float f = static_cast<float>(std::clamp(t, 0.0, 1.0)); return {a.r + (b.r - a.r) * f, a.g + (b.g - a.g) * f, a.b + (b.b - a.b) * f, a.a + (b.a - a.a) * f}; }
static bool inside(D2D1_RECT_F r, float x, float y) { return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom; }
static std::wstring percent(double value) { return std::to_wstring(static_cast<int>(std::round(value))) + L"%"; }

struct Options {
    fs::path config, transport, profile, screenshot, verify;
    TransportOptions network;
    bool endpointOverride = false, transportOverride = false, windowed = false, reduced = false;
};

class Panel {
public:
    State state;
    HWND hwnd = nullptr;
    float width = 1000, height = 700, dpi = 96;
    std::array<Motion, 8> motion{};
    std::unique_ptr<Sender> sender;
    uint64_t sequence = 0, draws = 0;
    bool fullscreen = false, reduced = false, animationTimer = false;
    int focus = 0;
private:
    Options options;
    ComPtr<ID2D1Factory> factory;
    ComPtr<IDWriteFactory5> textFactory;
    ComPtr<IDWriteInMemoryFontFileLoader> fontLoader;
    ComPtr<IDWriteFontCollection1> fonts;
    ComPtr<IWICImagingFactory> imaging;
    ComPtr<IWICFormatConverter> pixels;
    ComPtr<ID2D1HwndRenderTarget> target;
    ComPtr<ID2D1Bitmap> grip;
    ComPtr<ID2D1SolidColorBrush> brush;
    ComPtr<ID2D1LinearGradientBrush> paper;
    std::map<std::wstring, ComPtr<IDWriteTextLayout>> layouts;
    RECT savedRect{};
    int dragging = -1, pressedButton = -1;
    UINT32 pointer = 0;
    bool mouseDrag = false, keyboardEditing = false, showFocus = false;
    float offset = 0;
    double colorSince = -1000, rockerSince = -1000, rockerStart = 0;
    D2D1_COLOR_F accentStart = color(0xae3c32), railStart = color(0x592923);
    double profileStart = 0, phaseStart = 0, phaseCpu = 0;
    std::string phaseUtcStart;
    uint64_t phaseDraws = 0;
    int phase = -1, actions = 0, resets = 0;
    int warmSwitches = 0, warmUpdates = 0;
    bool warmReset = false;
    std::string phases;
    std::vector<double> drawIntervals;
    double previousDraw = 0;
    struct Geometry { float margin, right, top, headingSize, rulerY, bodyTop, row, bottom, labelWidth, sliderLeft, sliderRight, readoutLeft, gripSize; D2D1_RECT_F reset, toggle; } g{};

    void geometry() {
        g.margin = std::clamp(width * .03f, 20.f, 56.f); g.right = width - g.margin;
        g.top = std::clamp(height * .027f, 16.f, 32.f);
        g.headingSize = std::clamp(std::min(width * .058f, height * .086f), 28.f, 96.f);
        float headerBottom = g.top + g.headingSize * 1.25f + std::clamp(height * .013f, 6.f, 16.f);
        g.rulerY = headerBottom + std::clamp(height * .017f, 6.f, 18.f);
        g.bodyTop = g.rulerY + std::clamp(height * .032f, 16.f, 30.f) + 4;
        float footerHeight = std::clamp(height * .081f, 44.f, 78.f) + std::clamp(height * .014f, 6.f, 14.f);
        g.bottom = height - g.top - footerHeight;
        g.row = std::max(20.f, (g.bottom - g.bodyTop) / 8);
        float usable = width - 2 * g.margin;
        g.labelWidth = usable * .42f; g.sliderLeft = g.margin + g.labelWidth; g.sliderRight = g.sliderLeft + usable * .48f; g.readoutLeft = g.sliderRight;
        g.gripSize = std::min(std::clamp(height * .078f, 30.f, 84.f), g.row * .98f);
        float footY = g.bottom + 12, footBottom = height - g.top;
        g.reset = D2D1::RectF(g.margin, footY, g.margin + usable * .3f, footBottom);
        // Footer labels scale together at narrow widths.
        float modeWidth = std::min(usable * .62f, std::clamp(std::min(width * .026f, height * .048f), 18.f, 42.f) * 12.5f);
        float modeLeft = g.right - modeWidth;
        g.toggle = D2D1::RectF(modeLeft + modeWidth * .36f, footY, modeLeft + modeWidth * .60f, footBottom);
    }
    D2D1_COLOR_F accent(double t) const { return mix(accentStart, color(state.cooling ? 0x287bb9 : 0xae3c32), (t - colorSince) / 250); }
    D2D1_COLOR_F rail(double t) const { return mix(railStart, color(state.cooling ? 0x203f64 : 0x592923), (t - colorSince) / 250); }
    double rocker(double t) const { return rockerStart + ((state.cooling ? 1.0 : 0.0) - rockerStart) * std::clamp((t - rockerSince) / 160, 0.0, 1.0); }
    void ink(ID2D1RenderTarget* rt, D2D1_COLOR_F c) { if (!brush) check(rt->CreateSolidColorBrush(c, &brush)); else brush->SetColor(c); }
    void rect(ID2D1RenderTarget* rt, D2D1_RECT_F r, D2D1_COLOR_F c) { ink(rt, c); rt->FillRectangle(r, brush.Get()); }
    void line(ID2D1RenderTarget* rt, float x1, float y1, float x2, float y2, D2D1_COLOR_F c, float thickness = 1) { ink(rt, c); rt->DrawLine(D2D1::Point2F(x1, y1), D2D1::Point2F(x2, y2), brush.Get(), thickness); }
    void text(ID2D1RenderTarget* rt, const std::wstring& s, float size, D2D1_RECT_F box, D2D1_COLOR_F c, int align = 0) {
        std::wstring key = s + L"\x1f" + std::to_wstring(static_cast<int>(size * 100));
        auto entry = layouts.find(key); ComPtr<IDWriteTextLayout> layout;
        if (entry != layouts.end()) layout = entry->second;
        else {
            ComPtr<IDWriteTextFormat> format;
            check(textFactory->CreateTextFormat(L"Russo One", fonts.Get(), DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, L"en-US", &format));
            check(format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP));
            check(textFactory->CreateTextLayout(s.c_str(), static_cast<UINT32>(s.size()), format.Get(), 4096, 512, &layout));
            if (layouts.size() > 512) layouts.clear(); layouts.emplace(key, layout);
        }
        DWRITE_TEXT_METRICS metrics{}; check(layout->GetMetrics(&metrics));
        float scale = std::min({1.f, (box.right - box.left) / std::max(1.f, metrics.widthIncludingTrailingWhitespace), (box.bottom - box.top) / std::max(1.f, metrics.height)});
        float x = box.left + (align == 2 ? box.right - box.left - metrics.widthIncludingTrailingWhitespace * scale : align == 1 ? (box.right - box.left - metrics.widthIncludingTrailingWhitespace * scale) / 2 : 0);
        float y = box.top + (box.bottom - box.top - metrics.height * scale) / 2;
        ink(rt, c); rt->SetTransform(D2D1::Matrix3x2F::Scale(scale, scale) * D2D1::Matrix3x2F::Translation(x, y));
        rt->DrawTextLayout(D2D1::Point2F(0, 0), layout.Get(), brush.Get()); rt->SetTransform(D2D1::Matrix3x2F::Identity());
    }
    void loadAssets() {
        check(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory.GetAddressOf()));
        check(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory5), reinterpret_cast<IUnknown**>(textFactory.GetAddressOf())));
        check(textFactory->CreateInMemoryFontFileLoader(&fontLoader)); check(textFactory->RegisterFontFileLoader(fontLoader.Get()));
        auto [font, fontSize] = resource(102); ComPtr<IDWriteFontFile> file;
        check(fontLoader->CreateInMemoryFontFileReference(textFactory.Get(), font, fontSize, nullptr, &file));
        ComPtr<IDWriteFontSetBuilder1> builder; check(textFactory->CreateFontSetBuilder(&builder)); check(builder->AddFontFile(file.Get()));
        ComPtr<IDWriteFontSet> set; check(builder->CreateFontSet(&set)); check(textFactory->CreateFontCollectionFromFontSet(set.Get(), &fonts));
        check(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&imaging)));
        auto [image, imageSize] = resource(101); ComPtr<IWICStream> stream; check(imaging->CreateStream(&stream));
        check(stream->InitializeFromMemory(reinterpret_cast<BYTE*>(const_cast<void*>(image)), imageSize));
        ComPtr<IWICBitmapDecoder> decoder; check(imaging->CreateDecoderFromStream(stream.Get(), nullptr, WICDecodeMetadataCacheOnLoad, &decoder));
        ComPtr<IWICBitmapFrameDecode> frame; check(decoder->GetFrame(0, &frame));
        UINT iw, ih; check(frame->GetSize(&iw, &ih)); ComPtr<IWICBitmapScaler> scaler; check(imaging->CreateBitmapScaler(&scaler));
        check(scaler->Initialize(frame.Get(), 192, std::max(1u, ih * 192 / iw), WICBitmapInterpolationModeFant));
        check(imaging->CreateFormatConverter(&pixels)); check(pixels->Initialize(scaler.Get(), GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, nullptr, 0, WICBitmapPaletteTypeCustom));
    }
    void discard() { paper.Reset(); brush.Reset(); grip.Reset(); target.Reset(); }
    void deviceResources() {
        if (target) return; RECT r{}; GetClientRect(hwnd, &r);
        if (r.right <= 0 || r.bottom <= 0) return;
        auto properties = D2D1::RenderTargetProperties(); properties.dpiX = properties.dpiY = dpi;
        check(factory->CreateHwndRenderTarget(properties, D2D1::HwndRenderTargetProperties(hwnd, D2D1::SizeU(r.right, r.bottom)), &target));
        check(target->CreateBitmapFromWicBitmap(pixels.Get(), &grip));
    }
    void ensurePaper(ID2D1RenderTarget* rt) {
        if (paper) return;
        D2D1_GRADIENT_STOP stops[] = {{0, color(0xeccd9e)}, {1, color(0xcfae7c)}};
        ComPtr<ID2D1GradientStopCollection> collection; check(rt->CreateGradientStopCollection(stops, 2, &collection));
        check(rt->CreateLinearGradientBrush(D2D1::LinearGradientBrushProperties(D2D1::Point2F(0, 0), D2D1::Point2F(width * .35f, height)), collection.Get(), &paper));
    }
    bool moving(double t) {
        bool active = false;
        for (auto& m : motion) { if (m.active && t - m.since >= m.duration()) m.active = false; active |= m.active; }
        return active || t - colorSince < 250 || t - rockerSince < 160;
    }
    void requestFrame() { InvalidateRect(hwnd, nullptr, FALSE); if (!animationTimer && moving(nowMs()) && !IsIconic(hwnd)) { SetTimer(hwnd, 1, 8, nullptr); animationTimer = true; } }
    void updateMotion(bool animate) { double t = nowMs(); for (size_t i = 0; i < 8; ++i) motion[i].retarget(state.bank()[i], t, animate && !reduced); requestFrame(); }
    void send(bool final = false) { ++sequence; sender->submit(state.payload(sequence, final), sequence, final); }
    void changeFocus(int next) { if (keyboardEditing && focus < 8) { state.commit(focus); motion[focus].retarget(state.bank()[focus], nowMs(), false); send(true); } keyboardEditing = false; focus = next; requestFrame(); }
    int hit(float x, float y) const {
        if (inside(g.reset, x, y)) return 8;
        if (inside(g.toggle, x, y)) return 9;
        if (x >= g.sliderLeft && x <= g.sliderRight && y >= g.bodyTop && y < g.bodyTop + 8 * g.row) return std::clamp(static_cast<int>((y - g.bodyTop) / g.row), 0, 7);
        return -1;
    }
    void updateDrag(float x) {
        if (dragging < 0) return;
        float travel = g.sliderRight - g.sliderLeft - g.gripSize;
        if (travel > 0 && state.set(dragging, (x - g.sliderLeft - g.gripSize / 2 - offset) / travel * 300)) {
            motion[dragging].retarget(state.bank()[dragging], nowMs(), false); send(); requestFrame();
        }
    }
    void begin(float x, float y, bool mouse, UINT32 pointerId = 0) {
        if (dragging >= 0 || pressedButton >= 0) return;
        int control = hit(x, y); if (control < 0) return;
        SetFocus(hwnd); changeFocus(control); showFocus = false;
        mouseDrag = mouse; pointer = pointerId;
        if (mouse) SetCapture(hwnd); // Touch/pen receive implicit pointer capture.
        if (control >= 8) pressedButton = control;
        else {
            dragging = control;
            double visible = motion[control].sample(nowMs());
            float center = g.sliderLeft + g.gripSize / 2 + static_cast<float>(visible / 300) * (g.sliderRight - g.sliderLeft - g.gripSize);
            offset = std::abs(x - center) <= g.gripSize / 2 ? x - center : 0;
            motion[control].retarget(visible, nowMs(), false); updateDrag(x);
        }
        requestFrame();
    }
    void finish(bool activate = false, float x = -1, float y = -1) {
        int button = pressedButton; pressedButton = -1;
        if (dragging >= 0) { int i = dragging; dragging = -1; state.commit(i); motion[i].retarget(state.bank()[i], nowMs(), false); send(true); }
        if (GetCapture() == hwnd) ReleaseCapture(); pointer = 0; mouseDrag = false;
        if (activate && button >= 8 && hit(x, y) == button) { if (button == 8) reset(); else toggle(); }
        requestFrame();
    }
    void readPreference() {
        BOOL enabled = TRUE; SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &enabled, 0); reduced = options.reduced || !enabled;
        if (reduced) { double t = nowMs(); colorSince = rockerSince = t - 1000; for (auto& m : motion) m.active = false; }
    }
public:
    Panel(Configuration config, Options o) : state(std::move(config)), options(std::move(o)) {
        for (size_t i = 0; i < 8; ++i) motion[i].target = state.bank()[i];
        loadAssets(); readPreference(); geometry();
    }
    ~Panel() { if (sender) sender->stop(); if (textFactory && fontLoader) textFactory->UnregisterFontFileLoader(fontLoader.Get()); }
    void attach(HWND h) {
        hwnd = h; dpi = static_cast<float>(GetDpiForWindow(h));
        sender = std::make_unique<Sender>(options.network, [h] { PostMessageW(h, NetworkChanged, 0, 0); });
        send(true);
    }
    void toggle() {
        finish(); changeFocus(focus); double t = nowMs(); accentStart = accent(t); railStart = rail(t); rockerStart = rocker(t);
        state.cooling = !state.cooling; colorSince = rockerSince = reduced ? t - 1000 : t; updateMotion(true);
        // A profile switch only changes the view, not the transmitted values.
    }
    void reset() { finish(); changeFocus(focus); state.reset(); updateMotion(true); send(true); }
    void fullscreenToggle() {
        if (!fullscreen) {
            GetWindowRect(hwnd, &savedRect); SetWindowLongPtrW(hwnd, GWL_STYLE, WS_POPUP | WS_VISIBLE);
            MONITORINFO monitor{}; monitor.cbSize = sizeof(monitor); GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &monitor);
            auto r = monitor.rcMonitor; SetWindowPos(hwnd, nullptr, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_FRAMECHANGED);
        } else {
            SetWindowLongPtrW(hwnd, GWL_STYLE, WS_OVERLAPPEDWINDOW | WS_VISIBLE);
            SetWindowPos(hwnd, nullptr, savedRect.left, savedRect.top, savedRect.right - savedRect.left, savedRect.bottom - savedRect.top, SWP_NOZORDER | SWP_FRAMECHANGED);
        }
        fullscreen = !fullscreen;
    }
    void resize() {
        RECT r; GetClientRect(hwnd, &r); width = r.right * 96.f / dpi; height = r.bottom * 96.f / dpi; geometry(); layouts.clear(); paper.Reset();
        if (target && r.right > 0 && r.bottom > 0) { auto hr = target->Resize(D2D1::SizeU(r.right, r.bottom)); if (FAILED(hr)) discard(); }
        requestFrame();
    }
    void draw(ID2D1RenderTarget* rt, ID2D1Bitmap* bitmap) {
        geometry(); double t = nowMs(); auto ac = accent(t), ra = rail(t); auto dark = color(0x47301e), muted = color(0x876749);
        ensurePaper(rt); rt->FillRectangle(D2D1::RectF(0, 0, width, height), paper.Get());
        ink(rt, color(0x785b3b, .4f)); rt->DrawRectangle(D2D1::RectF(14, 14, width - 14, height - 14), brush.Get());
        for (float x : {15.f, width - 15}) for (float y : {15.f, height - 15}) { ink(rt, color(0x555b52)); rt->FillEllipse(D2D1::Ellipse(D2D1::Point2F(x, y), 6.5f, 6.5f), brush.Get()); line(rt, x - 3, y + 3, x + 3, y - 3, color(0x171c1a)); }
        float headerBottom = g.rulerY - std::clamp(height * .017f, 6.f, 18.f);
        text(rt, L"ENGINEERING", g.headingSize, D2D1::RectF(g.margin, g.top, g.right, headerBottom - 6), dark);
        line(rt, g.margin, headerBottom, g.right, headerBottom, color(0x806443, .5f));
        if (options.profile.empty()) text(rt, wide(sender->getStatus()), 11, D2D1::RectF(g.right - 190, g.top, g.right, g.top + 16), muted, 2);
        auto allocation = allocate(state.coolant);
        float left = g.sliderLeft + g.gripSize / 2, right = g.sliderRight - g.gripSize / 2;
        float rulerFont = std::clamp(std::min(width * .014f, height * .027f), 10.f, 25.f);
        for (int i = 0; i < 4; ++i) {
            float x = left + (right - left) * i / 3;
            auto r = D2D1::RectF(x - 48, g.rulerY, x + 48, g.bodyTop - 4);
            if (i == 0) r.left = x; if (i == 3) r.right = x;
            text(rt, percent((state.cooling ? allocation.maximum : 300) * i / 3), rulerFont, r, color(0x735238), i == 0 ? 0 : i == 3 ? 2 : 1);
        }
        for (int i = 0; i < 8; ++i) {
            float top = g.bodyTop + i * g.row, y = top + g.row / 2;
            text(rt, labels[i], std::clamp(std::min(width * .033f, height * .058f), 18.f, 58.f), D2D1::RectF(g.margin, top, g.sliderLeft - 12, top + g.row), dark);
            float tick = std::min(22.f, g.row / 2 - 2);
            for (int j = 0; j < 4; ++j) { float x = left + (right - left) * j / 3; line(rt, x, y - tick, x, y + tick, color(0x6f5638, .25f)); }
            rect(rt, D2D1::RectF(left, y - 6.5f, right, y + 6.5f), ra);
            double visible = motion[i].sample(t);
            float handle = left + static_cast<float>(visible / 300) * (right - left);
            rect(rt, D2D1::RectF(left, y - 6.5f, left + static_cast<float>(std::clamp(visible, 0.0, 300.0) / 300) * (right - left), y + 6.5f), ac);
            line(rt, left, y - 6.5f, right, y - 6.5f, color(0x200f0a, .33f), 2);
            rt->DrawBitmap(bitmap, D2D1::RectF(handle - g.gripSize / 2, y - g.gripSize / 2, handle + g.gripSize / 2, y + g.gripSize / 2), 1, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
            text(rt, percent(state.cooling ? allocation.percentages[i] : state.power[i]), std::clamp(height * .043f, 18.f, 42.f), D2D1::RectF(g.readoutLeft + 4, top, g.right, top + g.row), ac, 2);
            if (showFocus && focus == i) { ink(rt, color(0x47301e, .8f)); rt->DrawRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(g.sliderLeft, top + 2, g.sliderRight, top + g.row - 2), 4, 4), brush.Get(), 1); }
        }
        line(rt, g.margin, g.bottom + 5, g.right, g.bottom + 5, color(0x806443, .44f));
        float diameter = std::min(std::clamp(height * .081f, 44.f, 78.f), g.reset.bottom - g.reset.top), cy = (g.reset.top + g.reset.bottom) / 2;
        auto circle = D2D1::Ellipse(D2D1::Point2F(g.margin + diameter / 2, cy), diameter / 2 - 3, diameter / 2 - 3);
        ink(rt, ac); rt->FillEllipse(circle, brush.Get()); ink(rt, ra); rt->DrawEllipse(circle, brush.Get(), 6);
        circle.radiusX -= 6; circle.radiusY -= 6; ink(rt, color(0xffffff, .53f)); rt->DrawEllipse(circle, brush.Get(), 2);
        text(rt, L"RESET\nLEVELS", std::clamp(height * .038f, 17.f, 38.f), D2D1::RectF(g.margin + diameter + 12, g.reset.top, g.reset.right, g.reset.bottom), ac);
        float modeWidth = std::min((width - 2 * g.margin) * .62f, std::clamp(std::min(width * .026f, height * .048f), 18.f, 42.f) * 12.5f);
        float modeLeft = g.right - modeWidth, modeSize = std::clamp(std::min(width * .026f, height * .048f), 18.f, 42.f);
        text(rt, L"POWER", modeSize, D2D1::RectF(modeLeft, g.toggle.top, g.toggle.left - 10, g.toggle.bottom), state.cooling ? muted : ac, 2);
        text(rt, L"COOLANT", modeSize, D2D1::RectF(g.toggle.right + 10, g.toggle.top, g.right, g.toggle.bottom), state.cooling ? ac : muted);
        float toggleW = std::min(82.f, g.toggle.right - g.toggle.left), toggleH = std::min(58.f, g.toggle.bottom - g.toggle.top);
        float cx = (g.toggle.left + g.toggle.right) / 2;
        auto housing = D2D1::RectF(cx - toggleW / 2, cy - toggleH / 2, cx + toggleW / 2, cy + toggleH / 2);
        ink(rt, color(0x353b32)); rt->FillRoundedRectangle(D2D1::RoundedRect(housing, 9, 9), brush.Get());
        housing.left += 7; housing.right -= 7; housing.top += 7; housing.bottom -= 7;
        ink(rt, color(0x264133)); rt->FillRoundedRectangle(D2D1::RoundedRect(housing, 4, 4), brush.Get());
        float lever = (housing.right - housing.left) * .4f, leverX = housing.left + 3 + static_cast<float>(rocker(t)) * (housing.right - housing.left - lever - 6);
        ink(rt, color(0xd1d6b6)); rt->FillRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(leverX, housing.top + 3, leverX + lever, housing.bottom - 3), 2, 2), brush.Get());
        if (showFocus && focus >= 8) { ink(rt, dark); rt->DrawRoundedRectangle(D2D1::RoundedRect(focus == 8 ? g.reset : g.toggle, 5, 5), brush.Get(), 2); }
    }
    void paint() {
        if (IsIconic(hwnd)) return; deviceResources(); if (!target) return;
        double t = nowMs(); target->BeginDraw(); draw(target.Get(), grip.Get()); auto result = target->EndDraw(); ++draws;
        if (phase >= 0 && previousDraw != 0) drawIntervals.push_back(t - previousDraw); previousDraw = t;
        if (result == D2DERR_RECREATE_TARGET) { discard(); requestFrame(); } else check(result);
    }
    void screenshot(const fs::path& path) {
        if (!path.parent_path().empty()) fs::create_directories(path.parent_path());
        ComPtr<IWICBitmap> bitmap; check(imaging->CreateBitmap(static_cast<UINT>(width * dpi / 96), static_cast<UINT>(height * dpi / 96), GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad, &bitmap));
        ComPtr<ID2D1RenderTarget> rt; auto properties = D2D1::RenderTargetProperties(); properties.dpiX = properties.dpiY = dpi;
        check(factory->CreateWicBitmapRenderTarget(bitmap.Get(), properties, &rt));
        ComPtr<ID2D1Bitmap> image; check(rt->CreateBitmapFromWicBitmap(pixels.Get(), &image));
        paper.Reset(); brush.Reset(); rt->BeginDraw(); draw(rt.Get(), image.Get()); check(rt->EndDraw()); paper.Reset(); brush.Reset();
        ComPtr<IWICStream> stream; check(imaging->CreateStream(&stream)); check(stream->InitializeFromFilename(path.c_str(), GENERIC_WRITE));
        ComPtr<IWICBitmapEncoder> encoder; check(imaging->CreateEncoder(GUID_ContainerFormatPng, nullptr, &encoder)); check(encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache));
        ComPtr<IWICBitmapFrameEncode> frame; check(encoder->CreateNewFrame(&frame, nullptr)); check(frame->Initialize(nullptr));
        check(frame->SetSize(static_cast<UINT>(width * dpi / 96), static_cast<UINT>(height * dpi / 96))); WICPixelFormatGUID format = GUID_WICPixelFormat32bppBGRA; check(frame->SetPixelFormat(&format)); check(frame->WriteSource(bitmap.Get(), nullptr)); check(frame->Commit()); check(encoder->Commit());
    }
    void key(WPARAM key) {
        if (key == VK_F11) { fullscreenToggle(); return; }
        if (key == VK_TAB) { finish(); showFocus = true; changeFocus((focus + (GetKeyState(VK_SHIFT) < 0 ? 9 : 1)) % 10); return; }
        if (focus >= 8 && (key == VK_SPACE || key == VK_RETURN)) { if (focus == 8) reset(); else toggle(); return; }
        if (focus >= 8) return;
        int value = state.bank()[focus];
        switch (key) { case VK_LEFT: case VK_DOWN: --value; break; case VK_RIGHT: case VK_UP: ++value; break; case VK_PRIOR: value += 10; break; case VK_NEXT: value -= 10; break; case VK_HOME: value = 0; break; case VK_END: value = 300; break; default: return; }
        keyboardEditing = true; showFocus = true; state.set(focus, value); motion[focus].retarget(state.bank()[focus], nowMs(), false); send(); requestFrame();
    }
    void verify() {
        int checks = 0; auto assertThat = [&](bool b, const char* s) { if (!b) throw std::runtime_error(std::string("UI check failed: ") + s); ++checks; };
        auto original = state; bool oldReduced = reduced; reduced = true;
        focus = 0; state.cooling = false; state.power[0] = 100; updateMotion(false);
        key(VK_RIGHT); assertThat(state.power[0] == 101, "keyboard unsnapped step");
        key(VK_TAB); assertThat(state.power[0] == 100, "focus-loss snap");
        auto bank = state.power; toggle(); assertThat(state.power == bank && state.cooling, "profile independence");
        state.coolant.fill(300); updateMotion(false); auto pool = allocate(state.coolant);
        assertThat(pool.maximum == 12.5 && pool.percentages[0] == 12.5, "shared coolant ruler");
        auto weights = state.coolant; state.set(0, 100); assertThat(state.coolant[1] == weights[1], "coolant weights stay independent");
        state.coolant[0] = 149; reset(); assertThat(state.coolant == state.config.coolant && state.power == bank, "active-only reset");
        toggle(); geometry(); float y = g.bodyTop + g.row / 2;
        begin(g.sliderRight - g.gripSize / 2, y, true); assertThat(state.power[0] == 300, "rail endpoint");
        updateDrag(g.sliderLeft - 100); finish(); assertThat(state.power[0] == 0 && dragging == -1 && GetCapture() != hwnd, "capture and bounds");
        auto before = state.power; begin(g.sliderLeft + g.gripSize / 2, y, false, 42); updateDrag(g.sliderRight); finish(); assertThat(state.power[0] == 300 && pointer == 0, "pointer handler release");
        assertThat(before[1] == state.power[1], "other slider preserved");
        fullscreenToggle(); assertThat(fullscreen, "enter fullscreen"); fullscreenToggle(); assertThat(!fullscreen, "leave fullscreen");
        state = original; reduced = oldReduced; focus = 0; keyboardEditing = false; colorSince = rockerSince = nowMs() - 1000; updateMotion(false);
        for (int i = 0; i < 120; ++i) { toggle(); if (i % 7 == 0) reset(); }
        assertThat(!state.cooling, "rapid toggle parity");
        reduced = false; toggle(); motion[0].since -= 30; auto at = nowMs(); auto visible = motion[0].sample(at); toggle();
        assertThat(motion[0].active && std::abs(motion[0].start - visible) < 10, "animation retarget preserves visible position");
        reduced = oldReduced;
        state = original; showFocus = false; updateMotion(false); colorSince = rockerSince = nowMs() - 1000;
        if (animationTimer) { KillTimer(hwnd, 1); animationTimer = false; }
        assertThat(!moving(nowMs()), "settled animation detachment");
        screenshot(options.screenshot.empty() ? options.verify.parent_path() / L"cpp-panel.png" : options.screenshot);
        auto resizeClient = [&](int w, int h) {
            RECT r{0, 0, static_cast<LONG>(w * dpi / 96), static_cast<LONG>(h * dpi / 96)};
            AdjustWindowRectExForDpi(&r, static_cast<DWORD>(GetWindowLongPtrW(hwnd, GWL_STYLE)), FALSE, 0, static_cast<UINT>(dpi));
            SetWindowPos(hwnd, nullptr, 40, 40, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            assertThat(g.bodyTop + 8 * g.row <= g.reset.top && g.gripSize <= g.row, "rows and footer fit on resize");
        };
        resizeClient(640, 480); screenshot(options.verify.parent_path() / L"cpp-panel-minimum.png");
        resizeClient(768, 1024); screenshot(options.verify.parent_path() / L"cpp-panel-portrait.png");
        state.cooling = true; colorSince = rockerSince = nowMs() - 1000; updateMotion(false); screenshot(options.verify.parent_path() / L"cpp-panel-coolant.png");
        state = original; resizeClient(1000, 700);
        writeFile(options.verify, "{\"passed\":true,\"checks\":" + std::to_string(checks) + ",\"physicalInputTested\":false,\"width\":" + std::to_string(width) + ",\"height\":" + std::to_string(height) + ",\"deviceScaleFactor\":" + std::to_string(dpi / 96) + "}");
    }
    void beginProfile() { profileStart = nowMs(); phase = -1; SetTimer(hwnd, 2, 6000, nullptr); }
    void profileTick() {
        double t = nowMs(), elapsed = t - profileStart;
        auto schedule = [&](double absolute) { SetTimer(hwnd, 2, static_cast<UINT>(std::max(1.0, std::ceil(absolute - nowMs()))), nullptr); };
        if (elapsed < 6000) { schedule(profileStart + 6000); return; }
        int desiredSwitches = std::clamp(static_cast<int>((elapsed - 6000) / 100), 0, 30);
        while (warmSwitches < desiredSwitches) { ++warmSwitches; toggle(); if (warmSwitches % 7 == 0) reset(); }
        if (elapsed < 9000) { schedule(profileStart + 6000 + (warmSwitches + 1) * 100); return; }
        int desiredUpdates = std::clamp(static_cast<int>((elapsed - 9000) / (2000.0 / 60)), 0, 60);
        while (warmUpdates < desiredUpdates) { ++warmUpdates; state.set(0, std::round((.5 + .48 * std::sin(warmUpdates * 3.141592653589793 / 30)) * 300)); updateMotion(false); }
        if (elapsed < 11000) { schedule(profileStart + 9000 + (warmUpdates + 1) * (2000.0 / 60)); return; }
        if (!warmReset) { state.set(0, 100); reset(); warmReset = true; }
        if (elapsed < 17000) { schedule(profileStart + 17000); return; }
        static const int durations[] = {5000, 12000, 5000, 10000};
        static const char* names[] = {"idle", "switching", "settled", "dragging"};
        if (phase == -1) { phase = 0; actions = resets = 0; phaseStart = t; phaseUtcStart = utcNow(); phaseCpu = cpuMs(); phaseDraws = draws; drawIntervals.clear(); previousDraw = 0; }
        double phaseElapsed = t - phaseStart;
        int count = phase == 1 ? 120 : phase == 3 ? 300 : 0;
        int desired = std::min(count, static_cast<int>(phaseElapsed * count / durations[phase]));
        while (actions < desired) {
            ++actions;
            if (phase == 1) { toggle(); if (actions % 7 == 0) { reset(); ++resets; } }
            else { state.set(0, std::round((.5 + .48 * std::sin(actions * 3.141592653589793 / 30)) * 300)); updateMotion(false); }
        }
        if (phaseElapsed < durations[phase]) { schedule(phaseStart + (count ? (actions + 1) * durations[phase] / static_cast<double>(count) : durations[phase])); return; }
        PROCESS_MEMORY_COUNTERS_EX memory{}; memory.cb = sizeof(memory); GetProcessMemoryInfo(GetCurrentProcess(), reinterpret_cast<PROCESS_MEMORY_COUNTERS*>(&memory), sizeof(memory));
        if (!phases.empty()) phases += ',';
        phases += "{\"start\":" + quote(phaseUtcStart) + ",\"end\":" + quote(utcNow()) + ",\"pids\":[{\"id\":" + std::to_string(GetCurrentProcessId()) + ",\"role\":\"C++ Direct2D\"}],";
        auto intervals = drawIntervals; std::sort(intervals.begin(), intervals.end()); auto percentile = [&](double f) { return intervals.empty() ? 0 : intervals[static_cast<size_t>((intervals.size() - 1) * f)]; };
        phases += "\"name\":" + quote(names[phase]) + ",\"elapsedMs\":" + std::to_string(phaseElapsed) + ",\"actions\":" + std::to_string(actions) + ",\"resets\":" + std::to_string(resets) + ",\"cpuMs\":" + std::to_string(cpuMs() - phaseCpu) + ",\"privateMiB\":" + std::to_string(memory.PrivateUsage / 1048576.0) + ",\"workingSetMiB\":" + std::to_string(memory.WorkingSetSize / 1048576.0) + ",\"draws\":" + std::to_string(draws - phaseDraws) + ",\"drawIntervalMedianMs\":" + std::to_string(percentile(.5)) + ",\"drawIntervalP95Ms\":" + std::to_string(percentile(.95)) + "}";
        if (++phase == 4) {
            SYSTEM_INFO info{}; GetSystemInfo(&info);
            writeFile(options.profile, "{\"implementation\":\"cpp-direct2d\",\"width\":" + std::to_string(width) + ",\"height\":" + std::to_string(height) + ",\"deviceScaleFactor\":" + std::to_string(dpi / 96) + ",\"logicalProcessors\":" + std::to_string(info.dwNumberOfProcessors) + ",\"reducedMotion\":" + (reduced ? "true" : "false") + ",\"transportEnabled\":false,\"frameMetric\":\"CPU draw callback intervals; not display presentation\",\"phases\":[" + phases + "]}");
            KillTimer(hwnd, 2); DestroyWindow(hwnd); return;
        }
        phaseStart = nowMs(); phaseUtcStart = utcNow(); phaseCpu = cpuMs(); actions = resets = 0; phaseDraws = draws; drawIntervals.clear(); previousDraw = 0;
        SetTimer(hwnd, 2, phase == 1 ? 100 : phase == 3 ? 33 : durations[phase], nullptr);
    }
    LRESULT message(UINT msg, WPARAM wp, LPARAM lp) {
        switch (msg) {
        case WM_PAINT: { PAINTSTRUCT ps; BeginPaint(hwnd, &ps); try { paint(); } catch (...) { EndPaint(hwnd, &ps); throw; } EndPaint(hwnd, &ps); return 0; }
        case WM_ERASEBKGND: return 1;
        case WM_SIZE: resize(); if (wp == SIZE_MINIMIZED && animationTimer) { KillTimer(hwnd, 1); animationTimer = false; } return 0;
        case WM_DPICHANGED: { dpi = static_cast<float>(HIWORD(wp)); if (target) target->SetDpi(dpi, dpi); auto r = reinterpret_cast<RECT*>(lp); SetWindowPos(hwnd, nullptr, r->left, r->top, r->right - r->left, r->bottom - r->top, SWP_NOZORDER | SWP_NOACTIVATE); return 0; }
        case WM_GETMINMAXINFO: { auto m = reinterpret_cast<MINMAXINFO*>(lp); m->ptMinTrackSize = {static_cast<LONG>(640 * dpi / 96), static_cast<LONG>(480 * dpi / 96)}; return 0; }
        case WM_TIMER:
            if (wp == 1) { InvalidateRect(hwnd, nullptr, FALSE); if (!moving(nowMs())) { KillTimer(hwnd, 1); animationTimer = false; } }
            else if (wp == 2) profileTick();
            else if (wp == 3) { KillTimer(hwnd, 3); if (!options.verify.empty()) verify(); else screenshot(options.screenshot); DestroyWindow(hwnd); }
            return 0;
        case WM_LBUTTONDOWN: begin(GET_X_LPARAM(lp) * 96.f / dpi, GET_Y_LPARAM(lp) * 96.f / dpi, true); return 0;
        case WM_MOUSEMOVE: if (mouseDrag && dragging >= 0) updateDrag(GET_X_LPARAM(lp) * 96.f / dpi); return 0;
        case WM_LBUTTONUP: if (mouseDrag) { updateDrag(GET_X_LPARAM(lp) * 96.f / dpi); finish(true, GET_X_LPARAM(lp) * 96.f / dpi, GET_Y_LPARAM(lp) * 96.f / dpi); } return 0;
        case WM_POINTERDOWN: case WM_POINTERUPDATE: case WM_POINTERUP: {
            UINT32 id = GET_POINTERID_WPARAM(wp); POINTER_INFO info{};
            if (!GetPointerInfo(id, &info)) break; if (info.pointerType == PT_MOUSE) break;
            POINT p = info.ptPixelLocation; ScreenToClient(hwnd, &p); float x = p.x * 96.f / dpi, y = p.y * 96.f / dpi;
            if (msg == WM_POINTERDOWN) begin(x, y, false, id);
            else if (pointer == id) { updateDrag(x); if (msg == WM_POINTERUP || (info.pointerFlags & POINTER_FLAG_CANCELED)) finish(msg == WM_POINTERUP && !(info.pointerFlags & POINTER_FLAG_CANCELED), x, y); }
            return 0;
        }
        case WM_POINTERCAPTURECHANGED: if (pointer == GET_POINTERID_WPARAM(wp)) finish(); return 0;
        case WM_CAPTURECHANGED: if (mouseDrag) finish(); return 0;
        case WM_CANCELMODE: finish(); return 0;
        case WM_KEYDOWN: key(wp); return 0;
        case WM_KILLFOCUS: finish(); changeFocus(focus); showFocus = false; return 0;
        case WM_ACTIVATE: if (LOWORD(wp) == WA_INACTIVE) { finish(); changeFocus(focus); } return 0;
        case WM_SETTINGCHANGE: readPreference(); requestFrame(); return 0;
        case NetworkChanged: requestFrame(); return 0;
        case WM_DESTROY: KillTimer(hwnd, 1); KillTimer(hwnd, 2); KillTimer(hwnd, 3); if (sender) sender->stop(); PostQuitMessage(0); return 0;
        }
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    static LRESULT CALLBACK procedure(HWND h, UINT msg, WPARAM wp, LPARAM lp) {
        auto self = reinterpret_cast<Panel*>(GetWindowLongPtrW(h, GWLP_USERDATA));
        if (msg == WM_NCCREATE) { self = reinterpret_cast<Panel*>(reinterpret_cast<CREATESTRUCTW*>(lp)->lpCreateParams); self->hwnd = h; SetWindowLongPtrW(h, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self)); }
        if (!self) return DefWindowProcW(h, msg, wp, lp);
        try { return self->message(msg, wp, lp); }
        catch (const std::exception& e) {
            try { if (!self->options.verify.empty()) writeFile(self->options.verify, "{\"passed\":false,\"error\":" + quote(e.what()) + "}"); else if (!self->options.profile.empty()) writeFile(self->options.profile, "{\"error\":" + quote(e.what()) + "}"); else MessageBoxW(h, wide(e.what()).c_str(), L"Epsilon C++ error", MB_ICONERROR); } catch (...) { }
            DestroyWindow(h); PostQuitMessage(1); return 0;
        }
    }
    int run(HINSTANCE instance) {
        WNDCLASSW cls{}; cls.lpfnWndProc = procedure; cls.hInstance = instance; cls.lpszClassName = L"EpsilonCppPanel"; cls.hCursor = LoadCursorW(nullptr, IDC_ARROW); check(RegisterClassW(&cls) ? S_OK : HRESULT_FROM_WIN32(GetLastError()));
        bool diagnostic = !options.profile.empty() || !options.verify.empty() || !options.screenshot.empty();
        UINT initialDpi = GetDpiForSystem(); RECT r{0, 0, static_cast<LONG>((diagnostic ? 1000 : 1280) * initialDpi / 96), static_cast<LONG>((diagnostic ? 700 : 800) * initialDpi / 96)};
        DWORD style = !options.profile.empty() ? WS_POPUP : WS_OVERLAPPEDWINDOW; AdjustWindowRectExForDpi(&r, style, FALSE, 0, initialDpi);
        std::wstring title = L"Epsilon Engineering C++ · " + wide(state.config.ship);
        auto h = CreateWindowExW(0, cls.lpszClassName, title.c_str(), style, 40, 40, r.right - r.left, r.bottom - r.top, nullptr, nullptr, instance, this);
        if (!h) throw std::runtime_error("Cannot create controller window");
        attach(h); resize(); ShowWindow(h, SW_SHOW); UpdateWindow(h);
        if (!options.windowed && !diagnostic) fullscreenToggle();
        if (!options.profile.empty()) beginProfile();
        else if (diagnostic) SetTimer(h, 3, 500, nullptr);
        MSG m{}; int result; while ((result = GetMessageW(&m, nullptr, 0, 0)) > 0) { TranslateMessage(&m); DispatchMessageW(&m); }
        return result == -1 ? 1 : static_cast<int>(m.wParam);
    }
};

static int networkTest(const std::wstring& endpoint, const fs::path& output, const Configuration& config) {
    State state(config); Sender sender({endpoint, 30}); uint64_t seq = 1;
    sender.submit(state.payload(seq, false), seq, false);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    for (int i = 0; i < 300; ++i) { state.set(0, i); ++seq; sender.submit(state.payload(seq, false), seq, false); std::this_thread::sleep_for(std::chrono::milliseconds(1)); }
    state.set(0, 147); state.commit(0); ++seq; sender.submit(state.payload(seq, true), seq, true);
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
    while (sender.acknowledged < seq && std::chrono::steady_clock::now() < deadline) std::this_thread::sleep_for(std::chrono::milliseconds(20));
    bool passed = sender.acknowledged == seq;
    writeFile(output, "{\"passed\":" + std::string(passed ? "true" : "false") + ",\"sent\":" + std::to_string(sender.sent.load()) + ",\"failed\":" + std::to_string(sender.failed.load()) + ",\"finalSequence\":" + std::to_string(seq) + ",\"acknowledged\":" + std::to_string(sender.acknowledged.load()) + ",\"status\":" + quote(sender.getStatus()) + "}");
    return passed ? 0 : 1;
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int) {
    fs::path report;
    try {
        check(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED));
        struct ComScope { ~ComScope() { CoUninitialize(); } } comScope;
        wchar_t executable[32768]; DWORD n = GetModuleFileNameW(nullptr, executable, 32768); if (!n || n >= 32768) throw std::runtime_error("Cannot locate application");
        auto base = fs::path(executable).parent_path(); Options o; o.config = base / L"config.json"; o.transport = base / L"transport.json";
        int argc; auto argv = CommandLineToArgvW(GetCommandLineW(), &argc); if (!argv) throw std::runtime_error("Cannot read arguments");
        std::vector<std::wstring> args(argv + 1, argv + argc); LocalFree(argv); std::wstring testEndpoint; fs::path testReport;
        for (size_t i = 0; i < args.size(); ++i) {
            auto arg = args[i]; auto next = [&]() { if (++i >= args.size()) throw std::runtime_error("Missing value for " + narrow(arg)); return args[i]; };
            if (arg == L"--windowed") o.windowed = true;
            else if (arg == L"--reduced-motion") o.reduced = true;
            else if (arg == L"--config") o.config = fs::absolute(next());
            else if (arg == L"--transport") { o.transport = fs::absolute(next()); o.transportOverride = true; }
            else if (arg == L"--endpoint") { o.network.endpoint = next(); o.endpointOverride = true; }
            else if (arg == L"--profile") { o.profile = fs::absolute(next()); report = o.profile; }
            else if (arg == L"--verify") { o.verify = fs::absolute(next()); report = o.verify; }
            else if (arg == L"--screenshot") o.screenshot = fs::absolute(next());
            else if (arg == L"--network-test") { testEndpoint = next(); testReport = fs::absolute(next()); report = testReport; }
            else throw std::runtime_error("Unknown argument: " + narrow(arg));
        }
        Configuration config;
        try { config = Configuration::parse(readFile(o.config)); } catch (const std::exception& e) { throw std::runtime_error("Configuration " + narrow(o.config.wstring()) + ": " + e.what()); }
        if (!testEndpoint.empty()) return networkTest(testEndpoint, testReport, config);
        if (o.transportOverride || fs::exists(o.transport)) {
            auto t = JsonReader(readFile(o.transport)).parse(); auto enabled = std::get_if<bool>(&t.at("enabled").value); if (!enabled) throw std::runtime_error("transport.enabled must be true or false");
            if (!o.endpointOverride && *enabled) { o.network.endpoint = wide(t.at("endpoint").text()); Endpoint validate(o.network.endpoint); }
            o.network.sendHz = t.at("sendHz").integer(1, 120);
        }
        if (o.endpointOverride) { Endpoint validate(o.network.endpoint); }
        // Diagnostics never contact a configured real server.
        if (!o.profile.empty() || !o.verify.empty() || !o.screenshot.empty()) o.network.endpoint.clear();
        Panel panel(std::move(config), std::move(o)); return panel.run(instance);
    } catch (const std::exception& e) {
        if (!report.empty()) { try { writeFile(report, "{\"error\":" + quote(e.what()) + "}"); } catch (...) {} }
        else MessageBoxW(nullptr, wide(e.what()).c_str(), L"Epsilon Engineering C++ — startup error", MB_OK | MB_ICONERROR);
        return 1;
    }
}
