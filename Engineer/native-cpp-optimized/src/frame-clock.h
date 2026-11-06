#pragma once
#include <dxgi.h>
#include <wrl/client.h>
#include <atomic>
#include <condition_variable>
#include <mutex>
#include <thread>

// One coalesced UI notification per output vblank. There is no idle polling.
// Direct2D presents immediately so it does not perform a second vblank wait.
class FrameClock {
    using Output = Microsoft::WRL::ComPtr<IDXGIOutput>;
    HWND window; UINT message;
    std::mutex mutex; std::condition_variable wake;
    bool enabled = false, stopping = false;
    double fallbackHz = 60;
    HMONITOR monitor = nullptr;
    Output output;
    std::atomic<bool> pending{false};
    std::thread worker;
    void run() {
        std::unique_lock lock(mutex);
        for (;;) {
            wake.wait(lock, [&] { return stopping || enabled; });
            if (stopping) return;
            auto selected = output; double hz = fallbackHz;
            lock.unlock();
            HRESULT result = selected ? selected->WaitForVBlank() : E_FAIL;
            lock.lock();
            if (FAILED(result)) {
                ++fallbackWaits;
                if (wake.wait_for(lock, std::chrono::duration<double>(1 / hz), [&] { return stopping || !enabled; })) continue;
            } else ++vblankWaits;
            if (stopping) return;
            if (!enabled) continue;
            if (!pending.exchange(true)) { ++notifications; if (!PostMessageW(window, message, 0, 0)) pending = false; }
            // Do not queue another notification until UI has consumed this one.
            if (pending) wake.wait(lock, [&] { return stopping || !enabled || !pending.load(); });
        }
    }
public:
    std::atomic<uint64_t> vblankWaits{0}, fallbackWaits{0}, notifications{0};
    FrameClock(HWND h, UINT msg) : window(h), message(msg), worker([this] { run(); }) {}
    ~FrameClock() { stop(); }
    void stop() { { std::lock_guard guard(mutex); stopping = true; } wake.notify_all(); if (worker.joinable()) worker.join(); }
    void active(bool value) { { std::lock_guard guard(mutex); enabled = value; } wake.notify_all(); }
    void consumed() { pending = false; wake.notify_all(); }
    bool isActive() { std::lock_guard guard(mutex); return enabled; }
    void selectMonitor(bool force = false) {
        auto selected = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        if (!force && selected == monitor) return;
        monitor = selected; Output chosen;
        Microsoft::WRL::ComPtr<IDXGIFactory1> factory;
        if (SUCCEEDED(CreateDXGIFactory1(IID_PPV_ARGS(&factory)))) {
            for (UINT i = 0; !chosen; ++i) {
                Microsoft::WRL::ComPtr<IDXGIAdapter1> adapter;
                if (factory->EnumAdapters1(i, &adapter) == DXGI_ERROR_NOT_FOUND) break;
                if (!adapter) break;
                for (UINT j = 0;; ++j) {
                    Output candidate; if (adapter->EnumOutputs(j, &candidate) == DXGI_ERROR_NOT_FOUND) break;
                    if (!candidate) break;
                    DXGI_OUTPUT_DESC desc{};
                    if (SUCCEEDED(candidate->GetDesc(&desc)) && desc.Monitor == selected) { chosen = candidate; break; }
                }
            }
        }
        double hz = 60; MONITORINFOEXW info{}; info.cbSize = sizeof(info);
        if (GetMonitorInfoW(selected, &info)) { DEVMODEW mode{}; mode.dmSize = sizeof(mode); if (EnumDisplaySettingsW(info.szDevice, ENUM_CURRENT_SETTINGS, &mode) && mode.dmDisplayFrequency > 1) hz = mode.dmDisplayFrequency; }
        { std::lock_guard guard(mutex); output = std::move(chosen); fallbackHz = hz; }
        wake.notify_all();
    }
};
