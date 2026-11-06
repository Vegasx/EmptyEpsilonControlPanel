// Functional rendering regression checks, invoked only by --verify.
template<class Assert>
void verifyOptimizations(Assert assertThat) {
    auto original = state; bool oldReduced = reduced;
    bool oldFocus = showFocus; int oldIndex = focus;
    auto pump = [&](double duration) {
        double until = nowMs() + duration;
        while (nowMs() < until) {
            MSG msg{};
            while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) {
                if (msg.message == WM_QUIT) throw std::runtime_error("Window closed during rendering checks");
                TranslateMessage(&msg); DispatchMessageW(&msg);
            }
            MsgWaitForMultipleObjects(0, nullptr, FALSE, 5, QS_ALLINPUT);
        }
    };
    reduced = true; showFocus = false; state.cooling = false; state.power = state.config.power;
    updateMotion(false); colorSince = rockerSince = nowMs() - 1000;
    // Exercise real HWND retained rendering through the window message loop.
    needsFull = true; requestFrame(); pump(100);
    auto fullBefore = fullDraws, partialBefore = partialDraws;
    reduced = false; toggle(); pump(340);
    assertThat(fullDraws == fullBefore && partialDraws > partialBefore, "profile switch uses partial HWND paints");
    assertThat(lastDrawArea < width * height * dpi / 96 * dpi / 96, "transition update area excludes unchanged panel");
    state.cooling = false; state.power = state.config.power; state.power[0] = 300;
    updateMotion(false); colorSince = rockerSince = nowMs() - 1000; requestFrame(); pump(100);
    reset(); double firstClock = motion[0].since; pump(25); reset();
    assertThat(motion[0].since == firstClock, "repeated reset preserves animation clock");
    assertThat(!motion[1].active && !motion[2].active, "reset does not animate unchanged sliders");
    pump(330); auto beforeResetDraws = draws; auto beforeSequence = sequence; reset(); pump(90);
    assertThat(draws == beforeResetDraws, "settled reset performs no redraw");
    assertThat(sequence > beforeSequence, "settled reset still submits final transport state");
    pump(80); auto idleNotifications = frameClock->notifications.load(); pump(120);
    assertThat(!frameClock->isActive() && frameClock->notifications == idleNotifications, "frame scheduler sleeps at idle");
    assertThat(frameClock->vblankWaits + frameClock->fallbackWaits > 0, "refresh scheduler delivered animation frames");

    // Compare partial rendering against an independently rebuilt full reference
    // image. This checks old-grip restoration, edge coverage and disjoint regions.
    struct Surface { ComPtr<IWICBitmap> pixels; ComPtr<ID2D1RenderTarget> rt; ComPtr<ID2D1Bitmap> gripImage; };
    auto makeSurface = [&]() {
        Surface s; check(imaging->CreateBitmap(static_cast<UINT>(width * dpi / 96), static_cast<UINT>(height * dpi / 96), GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad, &s.pixels));
        auto properties = D2D1::RenderTargetProperties(); properties.dpiX = properties.dpiY = dpi;
        check(factory->CreateWicBitmapRenderTarget(s.pixels.Get(), properties, &s.rt)); check(s.rt->CreateBitmapFromWicBitmap(pixels.Get(), &s.gripImage)); return s;
    };
    auto readPixels = [&](IWICBitmap* image) {
        UINT w, h; check(image->GetSize(&w, &h)); std::vector<BYTE> bytes(static_cast<size_t>(w) * h * 4);
        check(image->CopyPixels(nullptr, w * 4, static_cast<UINT>(bytes.size()), bytes.data())); return bytes;
    };
    float originalDpi = dpi;
    for (float testDpi : {96.f, 144.f, 192.f}) {
    dpi = testDpi;
    for (int scenario = 0; scenario < 5; ++scenario) {
        clearRenderCaches(); state = original; state.cooling = false; state.power.fill(100); state.coolant.fill(0);
        reduced = true; updateMotion(false); colorSince = rockerSince = nowMs() - 1000;
        auto surface = makeSurface(); ensureBackdrop(surface.rt.Get()); auto before = visual(nowMs());
        surface.rt->BeginDraw(); drawRegion(surface.rt.Get(), surface.gripImage.Get(), D2D1::RectF(0, 0, width, height), before); check(surface.rt->EndDraw());
        lastVisual = before; needsFull = false;
        auto after = before;
        if (scenario == 0) { after.positions[0] = 302; after.readouts[0] = L"300%"; }
        if (scenario == 1) { for (int i = 0; i < 8; ++i) { after.positions[i] = i % 2 ? -8 : 318; after.readouts[i] = L"0%"; } after.accent = mix(color(0xae3c32), color(0x287bb9), .47); after.rail = mix(color(0x592923), color(0x203f64), .47); after.cooling = true; after.lever = .61; }
        if (scenario == 2) { after.positions.fill(0); after.readouts.fill(L"0%"); after.focused = 4; }
        if (scenario == 3) { after.ruler = {L"0%", L"4%", L"8%", L"13%"}; after.readouts.fill(L"13%"); after.status = L"HTTP OK"; }
        if (scenario == 4) { after.positions[7] = -18; after.readouts[7] = L"0%"; after.focused = 8; }
        auto dirty = changedRegions(after); auto buildsBefore = backgroundBuilds;
        surface.rt->BeginDraw();
        for (auto r : dirty) { auto px = physical(r); drawRegion(surface.rt.Get(), surface.gripImage.Get(), D2D1::RectF(px.left * 96.f / dpi, px.top * 96.f / dpi, px.right * 96.f / dpi, px.bottom * 96.f / dpi), after); }
        check(surface.rt->EndDraw()); auto partialPixels = readPixels(surface.pixels.Get());
        assertThat(backgroundBuilds == buildsBefore, "partial frame reuses static background");
        auto masksBefore = maskBuilds;
        surface.rt->BeginDraw(); for (auto r : dirty) drawRegion(surface.rt.Get(), surface.gripImage.Get(), r, after); check(surface.rt->EndDraw());
        assertThat(maskBuilds == masksBefore, "transition text and reset masks are reused");
        clearRenderCaches(); auto reference = makeSurface(); ensureBackdrop(reference.rt.Get());
        reference.rt->BeginDraw(); drawRegion(reference.rt.Get(), reference.gripImage.Get(), D2D1::RectF(0, 0, width, height), after); check(reference.rt->EndDraw());
        auto referencePixels = readPixels(reference.pixels.Get());
        size_t different = 0; int largest = 0;
        for (size_t i = 0; i < partialPixels.size(); ++i) { int delta = std::abs(static_cast<int>(partialPixels[i]) - referencePixels[i]); if (delta > 2) ++different; largest = std::max(largest, delta); }
        if (different) throw std::runtime_error("Partial/full pixel mismatch at DPI " + std::to_string(dpi) + " in scenario " + std::to_string(scenario) + ": " + std::to_string(different) + " channels, max delta " + std::to_string(largest));
        assertThat(different == 0, "partial pixels match full reference");
    }
    }
    dpi = originalDpi;
    state = original; reduced = oldReduced; showFocus = oldFocus; focus = oldIndex;
    colorSince = rockerSince = nowMs() - 1000; updateMotion(false); clearRenderCaches(); requestFrame(); pump(70);
}
