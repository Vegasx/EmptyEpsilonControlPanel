#define NOMINMAX
#include <windows.h>
#include <dxgi.h>
#include <wrl/client.h>
#include <iostream>
#include <iomanip>
#include <sstream>
#include "model.h"
using Microsoft::WRL::ComPtr;
int main() {
    ComPtr<IDXGIFactory1> factory;
    if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory)))) return 1;
    std::cout << "{\"adapters\":["; bool comma = false;
    for (UINT i = 0;; ++i) {
        ComPtr<IDXGIAdapter1> adapter; auto status = factory->EnumAdapters1(i, &adapter);
        if (status == DXGI_ERROR_NOT_FOUND) break; if (FAILED(status)) return 1;
        DXGI_ADAPTER_DESC1 desc{}; if (FAILED(adapter->GetDesc1(&desc))) return 1;
        std::ostringstream luid; luid << std::hex << std::setfill('0') << "luid_0x" << std::setw(8) << static_cast<unsigned>(desc.AdapterLuid.HighPart) << "_0x" << std::setw(8) << desc.AdapterLuid.LowPart;
        int length = WideCharToMultiByte(CP_UTF8, 0, desc.Description, -1, nullptr, 0, nullptr, nullptr);
        std::string name(length, '\0'); WideCharToMultiByte(CP_UTF8, 0, desc.Description, -1, name.data(), length, nullptr, nullptr); name.pop_back();
        if (comma) std::cout << ','; comma = true;
        std::cout << "{\"description\":" << epsilon::quote(name) << ",\"luid\":" << epsilon::quote(luid.str()) << ",\"dedicatedMemoryMiB\":" << epsilon::number(desc.DedicatedVideoMemory / 1048576.0) << ",\"software\":" << ((desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) ? "true" : "false") << "}";
    }
    std::cout << "]}\n"; return 0;
}
