#ifndef UNICODE
#define UNICODE
#endif
#define NOMINMAX
#include <windows.h>
#include <pdh.h>
#include <pdhmsg.h>
#include <winperf.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <chrono>
#include <thread>
#include "model.h"
using namespace epsilon;
namespace fs = std::filesystem;
struct Query {
    PDH_HQUERY handle = nullptr;
    Query() { if (PdhOpenQueryW(nullptr, 0, &handle) != ERROR_SUCCESS) throw std::runtime_error("Cannot open PDH query"); }
    ~Query() { if (handle) PdhCloseQuery(handle); }
};
static double unixMs(LONGLONG time) { return static_cast<double>(time - 116444736000000000LL) / 10000; }
int wmain(int argc, wchar_t** argv) {
    try {
        if (argc != 3) throw std::runtime_error("Usage: gpu-sampler.exe <pid> <output.json>; create output.json.stop to stop");
        DWORD target = std::stoul(argv[1]); fs::path output(argv[2]); auto stop = output; stop += L".stop";
        if (!output.parent_path().empty()) fs::create_directories(output.parent_path());
        Query query; PDH_HCOUNTER counter = nullptr;
        auto status = PdhAddEnglishCounterW(query.handle, L"\\GPU Engine(*)\\Utilization Percentage", 0, &counter);
        if (status != ERROR_SUCCESS) throw std::runtime_error("GPU Engine counter unavailable: " + std::to_string(status));
        std::ofstream file(output, std::ios::binary); if (!file) throw std::runtime_error("Cannot write sampler output");
        file << "{\"pid\":" << target << ",\"counter\":\"GPU Engine / Utilization Percentage\",\"units\":\"raw active and elapsed time: 100ns\",\"intervalMs\":250,\"samples\":[";
        file.flush();
        fs::path ready = output; ready += L".ready"; { std::ofstream marker(ready); marker << "ready"; }
        std::map<std::wstring, PDH_RAW_COUNTER> previous;
        auto prefix = L"pid_" + std::to_wstring(target) + L"_";
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(75);
        auto next = std::chrono::steady_clock::now();
        bool comma = false; unsigned collects = 0, invalid = 0, resets = 0; DWORD counterType = 0;
        while (!fs::exists(stop) && std::chrono::steady_clock::now() < deadline) {
            auto result = static_cast<DWORD>(PdhCollectQueryData(query.handle)); ++collects;
            if (result != ERROR_SUCCESS && result != PDH_NO_DATA) throw std::runtime_error("PDH collect failed: " + std::to_string(result));
            DWORD size = 0, count = 0; auto read = static_cast<DWORD>(PdhGetRawCounterArrayW(counter, &size, &count, nullptr));
            if (read == PDH_MORE_DATA) {
                std::vector<unsigned char> bytes(size); auto items = reinterpret_cast<PDH_RAW_COUNTER_ITEM_W*>(bytes.data());
                read = static_cast<DWORD>(PdhGetRawCounterArrayW(counter, &size, &count, items));
                if (read == ERROR_SUCCESS) {
                    std::map<std::wstring, PDH_RAW_COUNTER> current;
                    std::string engines; LONGLONG from = 0, to = 0; unsigned matched = 0;
                    for (DWORD i = 0; i < count; ++i) {
                        std::wstring name = items[i].szName; if (!name.starts_with(prefix)) continue;
                        auto raw = items[i].RawValue;
                        if (raw.CStatus != PDH_CSTATUS_VALID_DATA && raw.CStatus != PDH_CSTATUS_NEW_DATA) { ++invalid; continue; }
                        current.emplace(name, raw); auto old = previous.find(name); if (old == previous.end()) continue;
                        auto dt = raw.SecondValue - old->second.SecondValue, busy = raw.FirstValue - old->second.FirstValue;
                        if (dt <= 0 || busy < 0) { ++resets; continue; }
                        if (from && (from != old->second.SecondValue || to != raw.SecondValue)) throw std::runtime_error("GPU engine clocks differ within a collection");
                        from = old->second.SecondValue; to = raw.SecondValue; ++matched;
                        if (busy) {
                            if (!engines.empty()) engines += ',';
                            engines += "{\"instance\":" + quote(std::string(name.begin(), name.end())) + ",\"busy100ns\":" + quote(std::to_string(busy)) + ",\"busyMs\":" + number(static_cast<double>(busy) / 10000) + ",\"percent\":" + number(100.0 * busy / dt) + "}";
                        }
                    }
                    previous = std::move(current);
                    if (matched) {
                        if (comma) file << ','; comma = true;
                        file << "{\"startMs\":" << number(unixMs(from)) << ",\"endMs\":" << number(unixMs(to)) << ",\"elapsed100ns\":" << quote(std::to_string(to - from)) << ",\"validEngines\":" << matched << ",\"active\":[" << engines << "]}";
                        file.flush();
                    }
                } else if (read != PDH_NO_DATA && read != PDH_CSTATUS_NO_INSTANCE) ++invalid;
            }
            next += std::chrono::milliseconds(250); std::this_thread::sleep_until(next);
        }
        DWORD infoSize = 0; PdhGetCounterInfoW(counter, FALSE, &infoSize, nullptr);
        std::vector<unsigned char> infoBuffer(infoSize); auto info = reinterpret_cast<PDH_COUNTER_INFO_W*>(infoBuffer.data());
        auto typeResult = PdhGetCounterInfoW(counter, FALSE, &infoSize, info); if (typeResult == ERROR_SUCCESS) counterType = info->dwType;
        file << "],\"collects\":" << collects << ",\"invalid\":" << invalid << ",\"counterResets\":" << resets << ",\"counterType\":" << counterType << ",\"typeReadStatus\":" << typeResult << "}";
        if (counterType != PERF_100NSEC_TIMER) throw std::runtime_error("Unexpected GPU counter type; raw-delta interpretation requires PERF_100NSEC_TIMER");
        return 0;
    } catch (const std::exception& error) { std::cerr << error.what() << '\n'; return 1; }
}
