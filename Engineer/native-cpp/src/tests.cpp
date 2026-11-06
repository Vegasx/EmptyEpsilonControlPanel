#include "model.h"
#include <fstream>
#include <iostream>
#include <random>
#include <sstream>
using namespace epsilon;
int main(int argc, char** argv) {
    int checks = 0;
    auto require = [&](bool b, const char* name) { if (!b) throw std::runtime_error(name); ++checks; };
    auto rejects = [&](const std::string& s) { try { JsonReader(s).parse(); } catch (...) { ++checks; return; } throw std::runtime_error("Accepted invalid JSON: " + s); };
    try {
        if (argc != 2) throw std::runtime_error("Expected config path");
        std::ifstream f(argv[1]); std::stringstream buffer; buffer << f.rdbuf(); auto source = buffer.str(); auto config = Configuration::parse(source);
        require(config.ship == "ESS Endeavour", "configuration");
        for (int i = 0; i <= 300; ++i) {
            int expected = i;
            bool strong = false;
            for (int stop : {100, 150, 200, 250, 300}) if (std::abs(i - stop) <= 3) { expected = stop; strong = true; break; }
            if (!strong) { int near = static_cast<int>(std::round(i / 5.0)) * 5; if (std::abs(i - near) <= 1) expected = near; }
            require(snap(i) == expected, "snap boundaries"); require(snap(snap(i)) == snap(i), "snap idempotence"); require(position(i) == i, "position identity");
        }
        require(position(-1000) == 0 && position(1000) == 300 && position(100.5) == 101, "rounding and clamp");
        std::mt19937 rng(1927);
        for (int n = 0; n < 1000; ++n) {
            Bank b{}; int total = 0; for (int& v : b) { v = rng() % 301; total += v; }
            auto before = b; auto a = allocate(b); double sum = 0;
            for (size_t i = 0; i < 8; ++i) { sum += a.percentages[i]; require(std::abs(a.percentages[i] - b[i] / static_cast<double>(std::max(300, total)) * 100) < 1e-10, "coolant proportions"); }
            require(sum <= 100.00000001 && total == a.total && b == before, "budget and immutable weights");
        }
        State s(config); s.set(0, 149); s.cooling = true; s.set(1, 210); s.reset(); require(s.power[0] == 149 && s.coolant == config.coolant, "active-only reset");
        s.config.coolant[0] = 149; s.reset(); require(s.coolant[0] == 149, "unsnapped defaults");
        s.cooling = false; s.commit(0); require(s.power[0] == 150, "commit snap");
        auto message = JsonReader(s.payload(42, true)).parse(); require(message.at("sequence").integer(42, 42) == 42 && message.at("powerPercentages").at("reactor").integer(150, 150) == 150, "wire contract");
        s.coolant.fill(0); s.coolant[0] = 1; auto precise = JsonReader(s.payload(43, false)).parse();
        require(std::get<double>(precise.at("coolant").at("percentages").at("reactor").value) == allocate(s.coolant).percentages[0], "full-precision coolant transport");
        Motion m; m.retarget(300, 0, true); require(std::abs(m.sample(170) - 330) < 1e-8, "overshoot"); require(m.sample(250) == 300, "exact endpoint"); double visible = m.sample(100); m.retarget(20, 100, true); require(std::abs(m.sample(100) - visible) < 1e-8, "interruption continuity"); m.retarget(50, 120, false); require(m.sample(120) == 50 && !m.active, "reduced motion");
        for (auto bad : {"", "{", "[]x", "{\"x\":1,\"x\":2}", "{\"a\":{\"b\":1,\"b\":2}}", "[1,]", "01", "+1", "1.", "1e", "NaN", "\"\\uD800\"", "\"\\uDC00\"", "\"a\nb\"", "{\"x\":true,}", "1e9999"}) rejects(bad);
        rejects(std::string("\"") + char(0xc0) + char(0x80) + "\"");
        require(JsonReader("\"\\uD83D\\uDE80\"").parse().text() == "\xf0\x9f\x9a\x80", "surrogate pair");
        require(JsonReader(quote("a\n\"\\")).parse().text() == "a\n\"\\", "JSON escaping");
        for (auto badValue : {"301", "-1", "1.5", "true", "\"100\""}) { auto bad = source; auto p = bad.find("100"); bad.replace(p, 3, badValue); bool rejected = false; try { Configuration::parse(bad); } catch (...) { rejected = true; } require(rejected, "config numeric validation"); }
        std::cout << "PASS: " << checks << " assertions (model, coolant, snapping, JSON, config, wire contract, motion)\n";
        return 0;
    } catch (const std::exception& e) { std::cerr << "FAIL: " << e.what() << '\n'; return 1; }
}
