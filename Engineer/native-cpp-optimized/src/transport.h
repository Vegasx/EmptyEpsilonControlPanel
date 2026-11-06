#pragma once
#include <windows.h>
#include <winhttp.h>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <optional>
#include <thread>
#include "model.h"

namespace epsilon {
inline std::wstring wide(std::string_view s) {
    if (s.empty()) return {};
    int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s.data(), static_cast<int>(s.size()), nullptr, 0);
    if (!n) throw std::runtime_error("Invalid UTF-8 text");
    std::wstring result(n, 0); MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s.data(), static_cast<int>(s.size()), result.data(), n); return result;
}
inline std::string narrow(std::wstring_view s) {
    if (s.empty()) return {};
    int n = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, s.data(), static_cast<int>(s.size()), nullptr, 0, nullptr, nullptr);
    if (!n) throw std::runtime_error("Invalid UTF-16 text");
    std::string result(n, 0); WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, s.data(), static_cast<int>(s.size()), result.data(), n, nullptr, nullptr); return result;
}
struct InternetHandle {
    HINTERNET value = nullptr;
    InternetHandle() = default; explicit InternetHandle(HINTERNET h) : value(h) { if (!h) throw std::runtime_error("WinHTTP error " + std::to_string(GetLastError())); }
    ~InternetHandle() { if (value) WinHttpCloseHandle(value); }
    InternetHandle(const InternetHandle&) = delete; InternetHandle& operator=(const InternetHandle&) = delete;
};
struct Endpoint {
    std::wstring host, path; INTERNET_PORT port; bool secure;
    explicit Endpoint(const std::wstring& url) {
        URL_COMPONENTS u{}; u.dwStructSize = sizeof(u); u.dwHostNameLength = u.dwUrlPathLength = u.dwExtraInfoLength = u.dwUserNameLength = u.dwPasswordLength = static_cast<DWORD>(-1);
        if (url.find(L'\0') != std::wstring::npos || !WinHttpCrackUrl(url.c_str(), static_cast<DWORD>(url.size()), 0, &u) ||
            (u.nScheme != INTERNET_SCHEME_HTTP && u.nScheme != INTERNET_SCHEME_HTTPS) || !u.dwHostNameLength || u.dwUserNameLength || u.dwPasswordLength || url.find(L'#') != std::wstring::npos)
            throw std::runtime_error("Endpoint must be an http:// or https:// URL without credentials or fragment");
        host.assign(u.lpszHostName, u.dwHostNameLength); path.assign(u.lpszUrlPath, u.dwUrlPathLength); if (path.empty()) path = L"/";
        if (u.dwExtraInfoLength) path.append(u.lpszExtraInfo, u.dwExtraInfoLength);
        port = u.nPort; secure = u.nScheme == INTERNET_SCHEME_HTTPS;
    }
};
struct TransportOptions { std::wstring endpoint; int sendHz = 30; };

// A synchronous WinHTTP session is confined to ONE background thread. Only that
// thread owns/closes its handles. UI submission is a bounded, latest-wins copy.
// Finite network timeouts bound shutdown; failures retain the newest full state.
class Sender {
    struct Delivery { std::string json; uint64_t sequence; bool final; };
    using Clock = std::chrono::steady_clock;
    TransportOptions options;
    std::mutex mutex; std::condition_variable wake;
    std::optional<Delivery> pending;
    bool stopping = false;
    std::string status = "LOCAL";
    std::function<void()> notify;
    std::thread worker;
    void setStatus(std::string next) {
        bool changed; { std::lock_guard guard(mutex); changed = status != next; status = std::move(next); }
        if (changed && notify) notify();
    }
    bool post(HINTERNET connection, const Endpoint& endpoint, const Delivery& delivery) {
        InternetHandle request(WinHttpOpenRequest(connection, L"POST", endpoint.path.c_str(), nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, endpoint.secure ? WINHTTP_FLAG_SECURE : 0));
        DWORD redirect = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        if (!WinHttpSetOption(request.value, WINHTTP_OPTION_REDIRECT_POLICY, &redirect, sizeof(redirect))) throw std::runtime_error("Cannot disable HTTP redirects");
        DWORD logon = WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH;
        WinHttpSetOption(request.value, WINHTTP_OPTION_AUTOLOGON_POLICY, &logon, sizeof(logon));
        constexpr wchar_t headers[] = L"Content-Type: application/json; charset=utf-8\r\nAccept: application/json\r\n";
        if (!WinHttpSendRequest(request.value, headers, static_cast<DWORD>(-1), const_cast<char*>(delivery.json.data()), static_cast<DWORD>(delivery.json.size()), static_cast<DWORD>(delivery.json.size()), 0) || !WinHttpReceiveResponse(request.value, nullptr))
            throw std::runtime_error("WinHTTP error " + std::to_string(GetLastError()));
        DWORD code = 0, bytes = sizeof(code);
        if (!WinHttpQueryHeaders(request.value, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_HEADER_NAME_BY_INDEX, &code, &bytes, WINHTTP_NO_HEADER_INDEX)) throw std::runtime_error("Cannot read HTTP status");
        if (code < 200 || code >= 300) throw std::runtime_error("HTTP " + std::to_string(code));
        // Drain a bounded small response to allow connection reuse. A controller
        // receiver should return 204, or a small JSON acknowledgement.
        char buffer[4096]; DWORD read = 0; size_t total = 0; auto deadline = Clock::now() + std::chrono::seconds(2);
        do {
            if (!WinHttpReadData(request.value, buffer, sizeof(buffer), &read)) break;
            total += read;
        } while (read && total < 65536 && Clock::now() < deadline);
        return true;
    }
    void run() {
        try {
            Endpoint endpoint(options.endpoint);
            InternetHandle session(WinHttpOpen(L"Epsilon-Cpp/0.1", WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0));
            if (!WinHttpSetTimeouts(session.value, 1000, 1000, 1000, 1000)) throw std::runtime_error("Cannot configure HTTP timeouts");
            InternetHandle connection(WinHttpConnect(session.value, endpoint.host.c_str(), endpoint.port, 0));
            auto nextSend = Clock::now(), retryAfter = nextSend;
            int retryMs = 250;
            for (;;) {
                Delivery delivery;
                {
                    std::unique_lock lock(mutex);
                    for (;;) {
                        if (stopping) return;
                        if (!pending) { wake.wait(lock); continue; }
                        auto due = std::max(retryAfter, pending->final ? Clock::time_point::min() : nextSend);
                        if (Clock::now() >= due) break;
                        wake.wait_until(lock, due);
                    }
                    delivery = std::move(*pending); pending.reset();
                }
                try {
                    post(connection.value, endpoint, delivery); ++sent; acknowledged = delivery.sequence;
                    retryMs = 250; retryAfter = Clock::now(); nextSend = retryAfter + std::chrono::nanoseconds((1000000000LL + options.sendHz - 1) / options.sendHz);
                    setStatus("HTTP OK");
                } catch (const std::exception& error) {
                    ++failed;
                    { std::lock_guard guard(mutex); if (!pending) pending = std::move(delivery); }
                    retryAfter = Clock::now() + std::chrono::milliseconds(retryMs); retryMs = std::min(retryMs * 2, 4000);
                    setStatus("RETRY: " + std::string(error.what()));
                }
            }
        } catch (const std::exception& error) { setStatus("HTTP ERROR: " + std::string(error.what())); }
    }
public:
    std::atomic<uint64_t> sent{0}, failed{0}, acknowledged{0};
    explicit Sender(TransportOptions o, std::function<void()> callback = {}) : options(std::move(o)), notify(std::move(callback)) {
        if (options.sendHz < 1 || options.sendHz > 120) throw std::runtime_error("sendHz must be 1..120");
        if (!options.endpoint.empty()) { Endpoint validate(options.endpoint); status = "CONNECTING"; worker = std::thread([this] { run(); }); }
    }
    ~Sender() { stop(); }
    void stop() { { std::lock_guard guard(mutex); stopping = true; } wake.notify_all(); if (worker.joinable()) worker.join(); }
    void submit(std::string json, uint64_t sequence, bool final) {
        if (options.endpoint.empty()) return;
        { std::lock_guard guard(mutex); if (stopping) return; pending = Delivery{std::move(json), sequence, final}; }
        wake.notify_one();
    }
    std::string getStatus() { std::lock_guard guard(mutex); return status; }
};
}
