// Independent numerical oracle: compile against the existing C++ implementation.
#include "../../native-cpp-optimized/src/model.h"
#include <fstream>
using namespace epsilon;
int main(int argc, char** argv) {
    if (argc != 2) return 1;
    std::ofstream out(argv[1]); out << "{\"positions\":[";
    for(int i=-400;i<=1600;++i) { double x=i/4.0; if(i!=-400)out<<','; out<<'['<<number(x)<<','<<position(x)<<','<<snap(x)<<']'; }
    out << "],\"motions\":["; bool first=true;
    for(auto pair: {std::pair{0.,300.}, {300.,0.}, {100.,150.}, {149.,150.}, {150.,150.}, {275.,25.}, {0.,1.}}) {
        if(!first) out<<','; first=false; Motion m; m.start=pair.first;m.target=pair.second;m.active=true;
        out<<'['<<number(m.start)<<','<<number(m.target)<<",[";
        for(int t=0;t<=300;++t){if(t)out<<',';out<<number(m.sample(t));}out<<"]]";
    }
    out<<"],\"allocations\":["; first=true;
    for(int n=0;n<512;++n) { Bank bank{};for(int i=0;i<8;++i)bank[i]=(n*(i+11)*37)%301;auto a=allocate(bank);if(!first)out<<',';first=false;out<<"[[";
        for(int i=0;i<8;++i){if(i)out<<',';out<<bank[i];}out<<"],"<<a.total<<','<<number(a.maximum)<<",[";
        for(int i=0;i<8;++i){if(i)out<<',';out<<number(a.percentages[i]);}out<<"]]";
    }out<<"]}";
}
