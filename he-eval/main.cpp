#include <iostream>
#include <map>
#include <string>

#include <fstream>
#include <sstream>
#include <sys/time.h>

#include <helib/helib.h>
#include <helib/binaryArith.h>
#include <helib/intraSlot.h>

#include "eqn_driver.hpp"
#include "utils.hpp"
#include "regalloc.hpp"

using namespace std;
using namespace eqn;

#define MEASURE_START(s) cout << "\e[1;34m" << s << "... "; flush(cout); TIC(t);
#define MEASURE_END cout << TOC(t) << "ms\e[0m" << endl;

class CircuitEvaluator {
    public:
        helib::Ctxt trueCt;
        helib::Ctxt falseCt;
        optional<helib::Ctxt>* registers;
        map<string, bool> plaintext_memory;
        const helib::PubKey &pk;
        const RegisterAllocator &ra;
        bool debug;

        CircuitEvaluator(vector<string> &inputlist, const helib::PubKey& pk, const RegisterAllocator& ra, bool debug=false) 
            : trueCt(pk), falseCt(pk), pk(pk), ra(ra), debug(debug) {
            registers = new optional<helib::Ctxt>[ra.colors]();

            for (auto &input: inputlist) {
                helib::Ctxt ct(pk);
                bool pt = (bool)(rand() % 2);

                if (debug) cout << "input :" << input << " : " << pt << endl;
                pk.Encrypt(ct, NTL::to_ZZX(pt));
                int reg = ra.net2reg(input);
                registers[reg] = ct;
                plaintext_memory[input] = pt;
            }
            pk.Encrypt(trueCt, NTL::to_ZZX(1));
            pk.Encrypt(falseCt, NTL::to_ZZX(0));
        }

        void evaluate(vector<tuple<string, Gate*>> &eqnlist) {
            for (auto &eqn : eqnlist) {
                string net = get<0>(eqn);
                int reg = ra.net2reg(net);
                Gate* gate = get<1>(eqn);

                bool newVal;

                switch (gate->op) {
                    case Gate::Op::AND: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        leftPair.first *= rightPair.first;
                        leftPair.first.reLinearize();
                        
                        registers[reg] = std::nullopt;
                        registers[reg] = leftPair.first;

                        newVal = leftPair.second && rightPair.second;
                        break;
                    }
                    case Gate::Op::OR: {
                        GateInp* left = gate->left;
                        left->polarity = !left->polarity;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        right->polarity = !right->polarity;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        // TODO: how does this compare to the Lobster way of doing it?
                        // L OR R = (L AND R) XOR (L XOR R)
                        leftPair.first *= rightPair.first;
                        leftPair.first += trueCt;
                        leftPair.first.reLinearize();

                        registers[reg] = std::nullopt;
                        registers[reg] = leftPair.first;

                        newVal = !(leftPair.second && rightPair.second);
                        break;
                    }
                    case Gate::Op::XOR: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        leftPair.first += rightPair.first;
                        registers[reg] = std::nullopt;
                        registers[reg] = leftPair.first;

                        newVal = leftPair.second ^ rightPair.second;
                        break;
                    }
                    case Gate::Op::WIRE: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);

                        registers[reg] = std::nullopt;
                        registers[reg] = leftPair.first;

                        newVal = leftPair.second;
                        break;
                    }
                }
                
                plaintext_memory[net] = newVal;
                if (debug) cout << "Evaluated " << net << endl;                
            }
        }

        void validate(vector<string> &outputlist, const helib::SecKey &sk) {
            for (auto &out: outputlist) {
                NTL::ZZX tmp_res;
                int reg = ra.net2reg(out);
                helib::Ctxt &ct = registers[reg].value();
                bool pt = plaintext_memory.find(out)->second;
                sk.Decrypt(tmp_res, ct);
                if (tmp_res[0] != pt) {
                    cerr << "Output " << out << " disagrees with plaintext: " << "CT " << tmp_res[0] << " vs PT " << pt << endl; 
                    exit(EXIT_FAILURE);
                }
            }
        }

        pair<helib::Ctxt, bool> evaluateGateInp(GateInp *gi) {
            helib::Ctxt resultCt(pk);
            bool resultPt = false;
            if (gi->type == GateInp::InpType::Const) {
                resultCt = gi->polarity ? trueCt : falseCt;
                resultPt = gi->polarity ? true : false;
            } else if (gi->type == GateInp::InpType::Var) {
                int reg = ra.net2reg(gi->name);
                
                auto &resultCtOpt = registers[reg];
                assert(resultCtOpt.has_value());
                resultCt = resultCtOpt.value();

                auto it = plaintext_memory.find(gi->name);
                assert(it != plaintext_memory.end());
                resultPt = it->second;

                if (!gi->polarity) {
                    resultCt += trueCt;
                    resultPt = !resultPt;
                }
            }
            return make_pair(resultCt, resultPt);
        }
};

long gateinp_md(GateInp *gi, map<string, long> &md_map) {
    if (gi->type == GateInp::InpType::Var) {
        auto it = md_map.find(gi->name);
        if (it == md_map.end()) {
            return 0;
        } else {
            return it->second;
        }
    } else {
        return 0;
    }
}

long find_md(vector<tuple<string, Gate*>> &eqnlist) {
    map<string, long> md_map;
    for (auto &eqn : eqnlist) {
        string net = get<0>(eqn);
        Gate* gate = get<1>(eqn);

        switch (gate->op) {
            case Gate::Op::AND: {
                long md = max(gateinp_md(gate->left, md_map), gateinp_md(gate->right, md_map)) + 1;

                md_map.insert(make_pair(net, md));
            }
            case Gate::Op::OR: {
                long md = max(gateinp_md(gate->left, md_map), gateinp_md(gate->right, md_map)) + 1;

                md_map.insert(make_pair(net, md));
            }
            case Gate::Op::XOR: {
                long md = max(gateinp_md(gate->left, md_map), gateinp_md(gate->right, md_map));

                md_map.insert(make_pair(net, md));
            }
            case Gate::Op::WIRE: {
                long md = gateinp_md(gate->left, md_map);

                md_map.insert(make_pair(net, md));
            }
        }
    }
    int max_md = 0;
    for (const auto & [key, md] : md_map) {
        if (md > max_md) { max_md = md; }
    }
    return max_md;
}

void help(const char* p_name) {
    cout << "Usage: " << p_name << " [OPTION]... [FILE] [DEPTH]" << endl << endl;
    cout << "   -d\t\t\tEnable debugging" << endl;
    cout << "   -q\t\t\tOnly print final duration" << endl;
}

int main( const int argc, const char **argv )
{
    if( argc < 2 )
    {
        help(argv[0]);
	    return EXIT_FAILURE;
    }
    // benchmarking variables
    TimeVar t;
    double processingTime(0.0);
    srand(time(NULL));

    EqnDriver driver;
    const int arglen = 1;
    const char* args[arglen];
    int argcnt = 0;
    bool debug = false;
    bool quiet = false;
    for (int i = 1; i < argc; i++) {
        switch (*(argv[i])) {
            case '-': {
                switch ((argv[i])[1]) {
                    case 'd': {debug = true; break;}
                    case 'q': {quiet = true; break;}
                    default: help(argv[0]); break;
                }
                break;
            }
            default: {
                if (argcnt >= arglen) { help(argv[0]); return EXIT_FAILURE; }
                args[argcnt] = argv[i];
                argcnt++;
                break;
            }
        }
    }

    if (argcnt != arglen) { 
        help(argv[0]);
        return EXIT_FAILURE;
    }

    driver.parse( args[0] );
    vector<string> inputlist = driver.inputlist;
    vector<string> outputlist = driver.outputlist;
    vector<tuple<string, Gate*>> eqnlist = driver.eqnlist;

    long depth = find_md(eqnlist);
    if (!quiet) cout << "Depth = " << depth << endl;

    //////////////////////////
    // Register Allocation //
    ////////////////////////
    if (!quiet) MEASURE_START("Allocate registers...")
    RegisterAllocator ra (inputlist, eqnlist, quiet);
    if (!quiet) MEASURE_END
    //exit(0);

    //////////////////////////
    // Setup HE parameters //
    ////////////////////////

    // 128-bit security
    long security = 128;

    // Number of bits in the modulus chain
    // Corresponds to depth of circuit
    long nBits = depth * 30; 

    // Number of columns of Key-Switching Matrix
    long c = 2; 

    // Plaintext modulus. 2 = boolean
    long p = 2;

    // ???
    long d = 1;

    // slot ???
    long s = 0;

    // Find m from desired parameters
    long m = helib::FindM(security, nBits, c, p, d, s, 0, debug);

    // Hensel lifting (default = 1)
    long r = 1;

    if (!quiet) MEASURE_START("Building context")
    helib::Context ctx = helib::ContextBuilder<helib::BGV>()
        .m(m)
        .p(p)
        .r(r)
        .bits(nBits)
        .c(c)
        .build();
    if (!quiet) MEASURE_END

    // Print the context
    if (!quiet) ctx.printout();

    ////////////////////
    // Generate keys //
    //////////////////
    helib::SecKey sk(ctx);

    if (!quiet) MEASURE_START("Generating secret key")
    sk.GenSecKey();
    if (!quiet) MEASURE_END

    if (!quiet) MEASURE_START("Add some 1D matrices")
    helib::addSome1DMatrices(sk);
    if (!quiet) MEASURE_END

    const helib::PubKey& pk = sk;

    ///////////////////////
    // Setup plaintexts //
    ///////////////////// 
    CircuitEvaluator ce(inputlist, pk, ra, debug);

    //////////////////////
    // Execute circuit //
    ////////////////////
    TIC(t);
    ce.evaluate(eqnlist);
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(timeNow() - t).count();
    auto hours = seconds / 3600;
    auto minutes = (seconds % 3600) / 60;
    seconds = seconds % 60;
    if (!quiet) cout << "Evaluated in ";
    cout << hours << "h " << minutes << "m " << seconds << "s " << endl;    

    ///////////////
    // Validate //
    /////////////
    ce.validate(outputlist, sk);

    return EXIT_SUCCESS;
}
