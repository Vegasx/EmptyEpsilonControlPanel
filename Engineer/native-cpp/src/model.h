#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <charconv>
#include <cstdint>
#include <limits>
#include <map>
#include <stdexcept>
#include <string>
#include <string_view>
#include <variant>
#include <vector>

namespace epsilon {
inline constexpr std::array<const char*, 8> ids = {"reactor", "beamweapons", "missilesystem", "maneuver", "impulse", "warp", "frontshield", "rearshield"};
inline constexpr std::array<const wchar_t*, 8> labels = {L"REACTOR", L"BEAM WEAPONS", L"MISSILE SYSTEM", L"MANEUVERING", L"IMPULSE ENGINES", L"WARP DRIVE", L"FRONT SHIELDS", L"REAR SHIELDS"};
using Bank = std::array<int, 8>;

// Small strict JSON reader, isolated from Win32 and shared with the test executable.
// No network or runtime package is required by the deployed application.
struct Json {
    using Object = std::map<std::string, Json>;
    using Array = std::vector<Json>;
    std::variant<std::nullptr_t, bool, double, std::string, Object, Array> value;
    const Json& at(const std::string& key) const {
        auto object = std::get_if<Object>(&value);
        if (!object || !object->contains(key)) throw std::runtime_error("Missing JSON property: " + key);
        return object->at(key);
    }
    bool has(const std::string& key) const { auto o = std::get_if<Object>(&value); return o && o->contains(key); }
    std::string text() const { if (auto s = std::get_if<std::string>(&value)) return *s; throw std::runtime_error("Expected JSON string"); }
    int integer(int low, int high) const {
        auto n = std::get_if<double>(&value);
        if (!n || !std::isfinite(*n) || *n < low || *n > high || std::floor(*n) != *n) throw std::runtime_error("Expected whole number in range " + std::to_string(low) + ".." + std::to_string(high));
        return static_cast<int>(*n);
    }
};
inline void utf8(std::string& s, unsigned c) {
    if (c <= 0x7f) s += static_cast<char>(c);
    else if (c <= 0x7ff) { s += static_cast<char>(0xc0 | (c >> 6)); s += static_cast<char>(0x80 | (c & 63)); }
    else if (c <= 0xffff) { s += static_cast<char>(0xe0 | (c >> 12)); s += static_cast<char>(0x80 | ((c >> 6) & 63)); s += static_cast<char>(0x80 | (c & 63)); }
    else { s += static_cast<char>(0xf0 | (c >> 18)); s += static_cast<char>(0x80 | ((c >> 12) & 63)); s += static_cast<char>(0x80 | ((c >> 6) & 63)); s += static_cast<char>(0x80 | (c & 63)); }
}
inline bool validUtf8(std::string_view s) {
    for (size_t i = 0; i < s.size();) {
        unsigned c = static_cast<unsigned char>(s[i++]);
        if (c < 128) continue;
        int n; unsigned min;
        if (c >= 0xc2 && c <= 0xdf) { n = 1; min = 0x80; c &= 31; }
        else if (c >= 0xe0 && c <= 0xef) { n = 2; min = 0x800; c &= 15; }
        else if (c >= 0xf0 && c <= 0xf4) { n = 3; min = 0x10000; c &= 7; }
        else return false;
        if (i + n > s.size()) return false;
        while (n--) { unsigned b = static_cast<unsigned char>(s[i++]); if ((b & 0xc0) != 0x80) return false; c = (c << 6) | (b & 63); }
        if (c < min || c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) return false;
    }
    return true;
}
class JsonReader {
    std::string_view s; size_t p = 0;
    [[noreturn]] void fail() const { throw std::runtime_error("Invalid JSON at byte " + std::to_string(p)); }
    void space() { while (p < s.size() && (s[p] == ' ' || s[p] == '\r' || s[p] == '\n' || s[p] == '\t')) ++p; }
    bool take(char c) { space(); if (p < s.size() && s[p] == c) { ++p; return true; } return false; }
    unsigned hex() { unsigned c = 0; for (int i = 0; i < 4; ++i) { if (p == s.size()) fail(); char b = s[p++]; c <<= 4; if (b >= '0' && b <= '9') c |= b - '0'; else if (b >= 'a' && b <= 'f') c |= b - 'a' + 10; else if (b >= 'A' && b <= 'F') c |= b - 'A' + 10; else fail(); } return c; }
    std::string string() {
        if (!take('"')) fail(); std::string r;
        while (p < s.size()) {
            unsigned char c = s[p++]; if (c == '"') return r; if (c < 32) fail();
            if (c != '\\') { r += static_cast<char>(c); continue; }
            if (p == s.size()) fail(); char e = s[p++];
            switch (e) {
            case '"': case '\\': case '/': r += e; break;
            case 'b': r += '\b'; break; case 'f': r += '\f'; break; case 'n': r += '\n'; break; case 'r': r += '\r'; break; case 't': r += '\t'; break;
            case 'u': { unsigned u = hex(); if (u >= 0xd800 && u <= 0xdbff) { if (p + 2 > s.size() || s[p++] != '\\' || s[p++] != 'u') fail(); unsigned l = hex(); if (l < 0xdc00 || l > 0xdfff) fail(); u = 0x10000 + ((u - 0xd800) << 10) + l - 0xdc00; } else if (u >= 0xdc00 && u <= 0xdfff) fail(); utf8(r, u); break; }
            default: fail();
            }
        }
        fail();
    }
    Json read(int depth) {
        if (depth > 64) fail(); space(); if (p == s.size()) fail();
        if (s[p] == '"') return Json{string()};
        if (take('{')) {
            Json::Object object; if (take('}')) return Json{object};
            do { auto key = string(); if (!take(':')) fail(); auto item = read(depth + 1); if (!object.emplace(key, std::move(item)).second) throw std::runtime_error("Duplicate JSON property: " + key); } while (take(','));
            if (!take('}')) fail(); return Json{object};
        }
        if (take('[')) { Json::Array a; if (take(']')) return Json{a}; do { a.push_back(read(depth + 1)); } while (take(',')); if (!take(']')) fail(); return Json{a}; }
        for (auto token : {std::string_view("true"), std::string_view("false"), std::string_view("null")}) {
            if (s.substr(p, token.size()) == token) { p += token.size(); if (token == "null") return Json{nullptr}; return Json{token == "true"}; }
        }
        size_t start = p; if (s[p] == '-') ++p;
        if (p == s.size()) fail();
        if (s[p] == '0') ++p;
        else { if (s[p] < '1' || s[p] > '9') fail(); while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p; }
        if (p < s.size() && s[p] == '.') { ++p; auto b = p; while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p; if (b == p) fail(); }
        if (p < s.size() && (s[p] == 'e' || s[p] == 'E')) { ++p; if (p < s.size() && (s[p] == '+' || s[p] == '-')) ++p; auto b = p; while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p; if (b == p) fail(); }
        try { double n = std::stod(std::string(s.substr(start, p - start))); if (!std::isfinite(n)) fail(); return Json{n}; } catch (...) { fail(); }
    }
public:
    explicit JsonReader(std::string_view input) : s(input) {}
    Json parse() { if (s.size() > 1024 * 1024 || !validUtf8(s)) throw std::runtime_error("JSON must be valid UTF-8 and at most 1 MiB"); if (s.starts_with("\xef\xbb\xbf")) p = 3; auto r = read(0); space(); if (p != s.size()) fail(); return r; }
};
inline std::string quote(std::string_view text) {
    std::string s = "\""; constexpr char h[] = "0123456789abcdef";
    for (unsigned char c : text) { if (c == '"' || c == '\\') { s += '\\'; s += c; } else if (c < 32) { s += "\\u00"; s += h[c >> 4]; s += h[c & 15]; } else s += c; }
    return s + '"';
}
inline std::string number(int value) { return std::to_string(value); }
inline std::string number(double value) {
    char buffer[128]; auto result = std::to_chars(buffer, buffer + sizeof(buffer), value, std::chars_format::general, std::numeric_limits<double>::max_digits10);
    if (result.ec != std::errc{}) throw std::runtime_error("Cannot serialize number");
    return {buffer, result.ptr};
}
struct Configuration {
    std::string ship; Bank power{}, coolant{};
    static Configuration parse(std::string_view text) {
        auto root = JsonReader(text).parse(); root.at("version").integer(1, 1);
        Configuration c; c.ship = root.at("shipName").text(); size_t units = 0; bool visible = false;
        for (unsigned char b : c.ship) { if ((b & 0xc0) != 0x80) { ++units; if (b >= 0xf0) ++units; } if (b > 32) visible = true; }
        if (!visible || units > 60 || c.ship.find('\0') != std::string::npos) throw std::runtime_error("shipName must contain 1-60 characters");
        for (size_t i = 0; i < ids.size(); ++i) { c.power[i] = root.at("defaults").at("power").at(ids[i]).integer(0, 300); c.coolant[i] = root.at("defaults").at("coolant").at(ids[i]).integer(0, 300); }
        return c;
    }
};
inline int position(double x) { if (!std::isfinite(x)) throw std::runtime_error("Non-finite slider position"); return static_cast<int>(std::clamp(std::round(x), 0.0, 300.0)); }
inline int snap(double x) { int n = position(x); for (int stop : {100, 150, 200, 250, 300}) if (std::abs(n - stop) <= 3) return stop; int nearest = static_cast<int>(std::round(n / 5.0)) * 5; return std::abs(n - nearest) <= 1 ? nearest : n; }
struct Allocation { int total = 0; double maximum = 100; std::array<double, 8> percentages{}; };
inline Allocation allocate(const Bank& weights) { Allocation a; for (int w : weights) a.total += w; double denominator = std::max(300, a.total); a.maximum = 300 / denominator * 100; for (size_t i = 0; i < 8; ++i) a.percentages[i] = weights[i] / denominator * 100; return a; }
struct State {
    Configuration config; Bank power, coolant; bool cooling = false;
    explicit State(Configuration c) : config(std::move(c)), power(config.power), coolant(config.coolant) {}
    Bank& bank() { return cooling ? coolant : power; }
    const Bank& bank() const { return cooling ? coolant : power; }
    bool set(size_t i, double x) { int n = position(x); bool changed = bank().at(i) != n; bank().at(i) = n; return changed; }
    void commit(size_t i) { bank().at(i) = snap(bank().at(i)); }
    void reset() { bank() = cooling ? config.coolant : config.power; }
    std::string payload(uint64_t sequence, bool final) const {
        auto a = allocate(coolant);
        auto object = [](const auto& values) { std::string s = "{"; for (size_t i = 0; i < 8; ++i) { if (i) s += ','; s += quote(ids[i]) + ':' + number(values[i]); } return s + '}'; };
        return "{\"version\":1,\"shipName\":" + quote(config.ship) + ",\"sequence\":" + std::to_string(sequence) + ",\"final\":" + (final ? "true" : "false") + ",\"powerPercentages\":" + object(power) + ",\"coolantWeights\":" + object(coolant) + ",\"coolant\":{\"totalWeight\":" + std::to_string(a.total) + ",\"rulerMaximum\":" + number(a.maximum) + ",\"percentages\":" + object(a.percentages) + "}}";
    }
};
struct Motion {
    double start = 0, target = 0, since = 0; bool active = false;
    double duration() const { return std::min(std::abs(target - start), 300.0) / 300 * 170 + 80; }
    double sample(double now) const {
        if (!active) return target;
        double d = target - start, t = std::min(std::abs(d), 300.0) / 300 * 170;
        std::array<double, 6> times{0, t, t + 24, t + 44, t + 62, t + 80};
        std::array<double, 6> values{start, target + d * .1, target - d * .01, target + d * .001, target - d * .0001, target};
        double elapsed = now - since;
        for (size_t i = 1; i < 6; ++i) if (elapsed < times[i]) return values[i - 1] + (values[i] - values[i - 1]) * std::clamp((elapsed - times[i - 1]) / (times[i] - times[i - 1]), 0.0, 1.0);
        return target;
    }
    void retarget(double value, double now, bool animate) { double current = sample(now); start = current; target = value; since = now; active = animate && std::abs(target - start) > .01; }
};
}
