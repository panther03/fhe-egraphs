#include <iostream>
#include <cstdlib>
#include <cstring>
#include <map>
#include <string>
#include <ctime>

#include <fstream>
#include <sstream>
#include <sys/time.h>

#include <helib/helib.h>
#include <helib/binaryArith.h>
#include <helib/intraSlot.h>

#include "eqn_driver.hpp"
#include "utils.hpp"

using namespace std;
using namespace eqn;

#define MEASURE_START(s) cout << "\e[1;34m" << s << "... "; flush(cout); TIC(t);
#define MEASURE_END cout << TOC(t) << "ms\e[0m" << endl;

class CircuitEvaluator {
    public:
        helib::Ctxt trueCt;
        helib::Ctxt falseCt;
        map<string, pair<helib::Ctxt, bool>> memory;
        const helib::PubKey &pk;

        CircuitEvaluator(vector<string> &inputlist, const helib::PubKey& pk) 
            : trueCt(pk), falseCt(pk), pk(pk) {
            for (auto &input: inputlist) {
                helib::Ctxt ct(pk);
                bool pt = (bool)(rand() % 2);

                cout << "input :" << input << " : " << pt << endl;
                pk.Encrypt(ct, NTL::to_ZZX(pt));
                memory.insert( make_pair(input, make_pair(ct, pt)));
            }
            pk.Encrypt(trueCt, NTL::to_ZZX(1));
            pk.Encrypt(falseCt, NTL::to_ZZX(0));
        }

        void Evaluate(vector<tuple<string, Gate*>> &eqnlist) {
            for (auto &eqn : eqnlist) {
                string net = get<0>(eqn);
                Gate* gate = get<1>(eqn);

                switch (gate->op) {
                    case Gate::Op::AND: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        leftPair.first *= rightPair.first;
                        leftPair.first.reLinearize();

                        leftPair.second = leftPair.second && rightPair.second;

                        memory.insert(make_pair(net, leftPair));
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

                        leftPair.second = !(leftPair.second && rightPair.second);

                        memory.insert(make_pair(net, leftPair));
                    }
                    case Gate::Op::XOR: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        leftPair.first += rightPair.first;

                        leftPair.second = leftPair.second && rightPair.second;

                        memory.insert(make_pair(net, leftPair));
                    }
                    case Gate::Op::WIRE: {
                        GateInp* left = gate->left;
                        pair<helib::Ctxt, bool> leftPair = evaluateGateInp(left);
                        GateInp* right = gate->right;
                        pair<helib::Ctxt, bool> rightPair = evaluateGateInp(right);

                        leftPair.first += rightPair.first;

                        leftPair.second = leftPair.second && rightPair.second;

                        memory.insert(make_pair(net, leftPair));
                    }
                }
            }
        }

        pair<helib::Ctxt, bool> evaluateGateInp(GateInp *gi) {
            helib::Ctxt resultCt(pk);
            bool resultPt = false;
            if (gi->type = GateInp::InpType::Const) {
                resultCt = gi->polarity ? trueCt : falseCt;
                resultPt = gi->polarity ? true : false;
            } else if (gi->type = GateInp::InpType::Var) {
                auto resultPair = memory.find(gi->name)->second;
                resultCt = resultPair.first;
                resultPt = resultPair.second;
                if (!gi->polarity) {
                    resultCt += trueCt;
                    resultPt = !resultPt;
                }
            }
            return make_pair(resultCt, resultPt);
        }
};

int main( const int argc, const char **argv )
{
    if( argc != 2 )
    {
	    return -1;
    }
    // benchmarking variables
    TimeVar t;
    double processingTime(0.0);
    srand(time(NULL));

    //int start_time = time(0);
    EqnDriver driver;

    driver.parse( argv[1] );
    vector<string> inputlist = driver.inputlist;
    vector<string> outputlist = driver.outputlist;
    vector<tuple<string, Gate*>> eqnlist = driver.eqnlist;

    long depth = 30;

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
    long m = helib::FindM(security, nBits, c, p, d, s, 0, true);

    // Hensel lifting (default = 1)
    long r = 1;

    MEASURE_START("Building context")
    helib::Context ctx = helib::ContextBuilder<helib::BGV>()
        .m(m)
        .p(p)
        .r(r)
        .bits(nBits)
        .c(c)
        .build();
    MEASURE_END

    // Print the context
    ctx.printout();

    ////////////////////
    // Generate keys //
    //////////////////
    helib::SecKey sk(ctx);

    MEASURE_START("Generating secret key")
    sk.GenSecKey();
    MEASURE_END

    MEASURE_START("Add some 1D matrices")
    helib::addSome1DMatrices(sk);
    MEASURE_END

    //MEASURE_START("Generating bootstrapping data")
    //sk.genRecryptData();
    //MEASURE_END

    const helib::PubKey& pk = sk;

    ///////////////////////
    // Setup plaintexts //
    ///////////////////// 
    CircuitEvaluator ce(inputlist, pk);

    //////////////////////
    // Execute circuit //
    ////////////////////
    ce.Evaluate(eqnlist);
    

/*    size_t poly_modulus_degree = 1024;
    parms.set_poly_modulus_degree(poly_modulus_degree);

    parms.set_coeff_modulus(CoeffModulus::BFVDefault(poly_modulus_degree));
    parms.set_*/
/*
    auto cc = BinFHEContext();

    cc.GenerateBinFHEContext(STD128_LMKCDEY, LMKCDEY);

    auto sk = cc.KeyGen();

    std::cout << "Generating the bootstrapping keys..." << std::endl;

    // Generate the bootstrapping keys (refresh and switching keys)
    cc.BTKeyGen(sk);

    std::cout << "Completed the key generation." << std::endl;

    auto ct1 = cc.Encrypt(sk, 1);
    auto ct2 = cc.Encrypt(sk, 1);

    // Compute (1 AND 1) = 1; Other binary gate options are OR, NAND, and NOR
    auto ctAND1 = cc.EvalBinGate(AND, ct1, ct2);

    // Compute (NOT 1) = 0
    auto ct2Not = cc.EvalNOT(ct2);

    // Compute (1 AND (NOT 1)) = 0
    auto ctAND2 = cc.EvalBinGate(AND, ct2Not, ct1);

    // Computes OR of the results in ctAND1 and ctAND2 = 1
    LWECiph ertext ctResult = cc.EvalBinGate(XOR, ctAND1, ctAND2);

    LWEPlaintext result;

    cc.Decrypt(sk, ctResult, &result);
    std::cout << "Result of encrypted computation of (1 AND 1) OR (1 AND (NOT 1)) = " << result << std::endl;
*/

/*
        vector<tuple<string, MC::Bexp*>> eqnlist = driver.eqnlist;
        
        
        // init HElib argument
        long m = 0, p = 2, r = 1;
        long depth = atoi(argv[2]);
        long L = depth * 30; //Level
        long c = 2;
        long w = 64;
        long d = 1;
        long security = 128;
        long s = 0;//slot
        ZZX G;
        
        m = FindM(security, L, c, p, d, s, 0);
	cout << "selected m : " << m << endl;
        FHEcontext context(m, p, r);
        buildModChain(context, L, c);
        FHESecKey sk(context);
        const FHEPubKey& pk = sk;
        
        G = context.alMod.getFactorsOverZZ()[0];

        sk.GenSecKey(w);

        addSome1DMatrices(sk);
        cout << "generated Key : " << endl;

        

        //encrypt inputlist, make memory
        map<string, Ctxt> memory;

        for(vector<string>::size_type i = 0; i < inputlist.size(); i++){
            Ctxt tmp(pk);
            int plaintext = 0;
            //plaintext modification
            if(i == 3 || i ==11)
                plaintext=1;
            cout << "input : " << inputlist[i] << " : " << plaintext << endl;
            pk.Encrypt(tmp, to_ZZX(plaintext));
            memory.insert( make_pair(inputlist[i], tmp) );
        }
        Ctxt true_ctxt(pk);
        pk.Encrypt(true_ctxt, to_ZZX(1));
        memory.insert( make_pair("true", true_ctxt) );

        Ctxt false_ctxt(pk);
        pk.Encrypt(false_ctxt, to_ZZX(0));
        memory.insert( make_pair("false", false_ctxt) );
         
        //for(auto i = memory.begin(); i!= memory.end(); i++){
        //    ZZX memory_res;
        //    sk.Decrypt(memory_res, i->second);
        //    cout << i->first << " : " << memory_res[0] << endl;
        //}
        
        for(auto i = 0 ; i < eqnlist.size(); i++){
            string lv = get<0>(eqnlist[i]);
            Bexp* bexp = get<1>(eqnlist[i]);
            auto top_op = bexp->head;
            int constant = bexp->constant;
            string var = bexp->var;
            Bexp* l_child = bexp->left;
            Bexp* r_child = bexp->right;
            if(top_op == MC::Bexp::Head::CONST){
                if(constant == 1){
                    memory.insert(make_pair(lv, true_ctxt ));
                }
                else if(constant == 0){
                    memory.insert(make_pair(lv, false_ctxt));
                }
            }
            else if(top_op == MC::Bexp::Head::VAR){
                memory.insert(make_pair(lv, memory.find(var)->second));
            }
            else if(top_op == MC::Bexp::Head::AND){
                auto lchild_op = l_child->head;
                string lchild_var = l_child->var;
                int lchild_const = l_child->constant;
                Ctxt lchild_ctxt(pk);
                if(lchild_op == MC::Bexp::Head::CONST){
                    if(lchild_const == 1){
                        lchild_ctxt = true_ctxt;
                    }
                    else{
                        lchild_ctxt = false_ctxt;
                    }
                }
                else if(lchild_op == MC::Bexp::Head::VAR){
                    lchild_ctxt = memory.find(lchild_var)->second;
                }

                auto rchild_op = r_child->head;
                string rchild_var = r_child->var;
                int rchild_const = r_child->constant;
                Ctxt rchild_ctxt(pk);
                if(rchild_op == MC::Bexp::Head::CONST){
                    if(rchild_const == 1){
                        rchild_ctxt = true_ctxt;
                    }
                    else{
                        rchild_ctxt = false_ctxt;
                    }
                }
                else if(rchild_op == MC::Bexp::Head::VAR){
                    rchild_ctxt = memory.find(rchild_var)->second;
                }
                // ZZX lchild_res;
                // ZZX rchild_res;
                // sk.Decrypt(lchild_res, lchild_ctxt);
                // sk.Decrypt(rchild_res, rchild_ctxt);

                lchild_ctxt *= rchild_ctxt;
                lchild_ctxt.reLinearize();               
                
                //ZZX and_res;
                //sk.Decrypt(and_res, lchild_ctxt);
                //cout << lchild_var << " * " << rchild_var << " = " << lv << endl;
                //cout << lchild_res[0] << " * " << rchild_res[0] << " = " << and_res[0] << endl;



                memory.insert(make_pair(lv, lchild_ctxt));
                
            }
            else if(top_op == MC::Bexp::Head::XOR){
                auto lchild_op = l_child->head;
                string lchild_var = l_child->var;
                int lchild_const = l_child->constant;
                Ctxt lchild_ctxt(pk);
                if(lchild_op == MC::Bexp::Head::CONST){
                    if(lchild_const == 1){
                        lchild_ctxt = true_ctxt;
                    }
                    else{
                        lchild_ctxt = false_ctxt;
                    }
                }
                else if(lchild_op == MC::Bexp::Head::VAR){
                    lchild_ctxt = memory.find(lchild_var)->second;
                }

                auto rchild_op = r_child->head;
                string rchild_var = r_child->var;
                int rchild_const = r_child->constant;
                Ctxt rchild_ctxt(pk);
                if(rchild_op == MC::Bexp::Head::CONST){
                    if(rchild_const == 1){
                        rchild_ctxt = true_ctxt;
                    }
                    else{
                        rchild_ctxt = false_ctxt;
                    }
                }
                else if(rchild_op == MC::Bexp::Head::VAR){
                    rchild_ctxt = memory.find(rchild_var)->second;
                }

                
                
                // ZZX lchild_res;
                // ZZX rchild_res;
                // sk.Decrypt(lchild_res, lchild_ctxt);
                // sk.Decrypt(rchild_res, rchild_ctxt);
                

                lchild_ctxt += rchild_ctxt;                

                // ZZX and_res;
                // sk.Decrypt(and_res, lchild_ctxt);
                // cout << lchild_var << " + " << rchild_var << " = " << lv << endl;
                // cout << lchild_res[0] << " + " << rchild_res[0] << " = " << and_res[0] << endl;

                memory.insert(make_pair(lv, lchild_ctxt));
                
            }
            else if(top_op == MC::Bexp::Head::OR){
                auto lchild_op = l_child->head;
                string lchild_var = l_child->var;
                int lchild_const = l_child->constant;
                Ctxt lchild_ctxt(pk);
                if(lchild_op == MC::Bexp::Head::CONST){
                    if(lchild_const == 1){
                        lchild_ctxt = true_ctxt;
                    }
                    else{
                        lchild_ctxt = false_ctxt;
                    }
                }
                else if(lchild_op == MC::Bexp::Head::VAR){
                    lchild_ctxt = memory.find(lchild_var)->second;
                }

                auto rchild_op = r_child->head;
                string rchild_var = r_child->var;
                int rchild_const = r_child->constant;
                Ctxt rchild_ctxt(pk);
                if(rchild_op == MC::Bexp::Head::CONST){
                    if(rchild_const == 1){
                        rchild_ctxt = true_ctxt;
                    }
                    else{
                        rchild_ctxt = false_ctxt;
                    }
                }
                else if(rchild_op == MC::Bexp::Head::VAR){
                    rchild_ctxt = memory.find(rchild_var)->second;
                }

                //ZZX lchild_res;
                //ZZX rchild_res;
                //sk.Decrypt(lchild_res, lchild_ctxt);
                //sk.Decrypt(rchild_res, rchild_ctxt);
                


                Ctxt tmp_ctxt1 = lchild_ctxt;
                tmp_ctxt1 += rchild_ctxt;
                lchild_ctxt *= rchild_ctxt;
                lchild_ctxt +=tmp_ctxt1;
                lchild_ctxt.reLinearize();

                //ZZX and_res;
                //sk.Decrypt(and_res, lchild_ctxt);
                //cout << lchild_var << " or " << rchild_var << " = " << lv << endl;
                //cout << lchild_res[0] << " or " << rchild_res[0] << " = " << and_res[0] << endl;

                memory.insert(make_pair(lv, lchild_ctxt));
                
            }
            else if(top_op == MC::Bexp::Head::NOT){
                auto rchild_op = r_child->head;
                string rchild_var = r_child->var;
                int rchild_const = r_child->constant;
                Ctxt rchild_ctxt(pk);
                if(rchild_op == MC::Bexp::Head::CONST){
                    if(rchild_const == 1){
                        rchild_ctxt = true_ctxt;
                    }
                    else{
                        rchild_ctxt = false_ctxt;
                    }
                }
                else if(rchild_op == MC::Bexp::Head::VAR){
                    rchild_ctxt = memory.find(rchild_var)->second;
                }
                
                //ZZX rchild_res;
                //sk.Decrypt(rchild_res, rchild_ctxt);

                rchild_ctxt += true_ctxt;

                //ZZX and_res;
                //sk.Decrypt(and_res, rchild_ctxt);
                //cout << "not " << rchild_var << " = " << lv << endl;
                //cout << "not " << rchild_res[0] << " = " << and_res[0] << endl;
                

                memory.insert(make_pair(lv, rchild_ctxt));
            }
            

        }
        cout << "circuit evaluation finished" << endl;
        for(auto i = 0; i < outputlist.size(); i++){
            ZZX tmp_res;
            sk.Decrypt(tmp_res, memory.find(outputlist[i])->second) ;
            cout << "output : " << outputlist[i] << " : " << tmp_res[0] << endl;   
        }

	int eval_time = time(0) - start_time;
	int consumed_hour = eval_time / 3600;
	int consumed_min = (eval_time % 3600) / 60;
	int consumed_sec = eval_time % 60;
	string time_string = "";
	if(0 < consumed_hour){
	  time_string += to_string(consumed_hour);
	  time_string += "h ";
	}

	if(0 < consumed_min && consumed_min < 10){
	  time_string += " ";
	  time_string += to_string(consumed_min);
	  time_string += "m ";
	}
	else if(10 <= consumed_min){
	  time_string += to_string(consumed_min);
	  time_string += "m ";
	}
  	else if(0 < consumed_hour)
    	  time_string += " 0m ";
 	
	if(consumed_sec < 10){
	  time_string += " ";
	  time_string += to_string(consumed_sec);
	  time_string += "s";
    	}
	else{
	  time_string += to_string(consumed_sec);
	  time_string += "s";
	}

	string circuit_filename = argv[1];
	string directory_tag = "paper_bench/";
	string empty_string = "";
        if(circuit_filename.find(directory_tag) != std::string::npos){
	    circuit_filename = circuit_filename.replace(circuit_filename.begin(), circuit_filename.begin() + 12, empty_string);
	}
	
	string opted_filename_tag = "opted_result";
	string baseline_filename_tag = "baseline";
	int name_length = circuit_filename.length();
	int time_length = time_string.length();

	
        if(circuit_filename.find(opted_filename_tag) != std::string::npos){
	    if(circuit_filename.find(baseline_filename_tag) == std::string::npos){
	      cerr << std::string(16 - time_length, ' ') << time_string;
	      cerr << endl;
	    }
	    else{
	      cerr << std::string(19 - time_length, ' ') << time_string; 
	    }
	}
	else{
	    cerr <<  circuit_filename << std::string(15 - name_length, ' ')  << std::string(12 - time_length, ' ') << time_string ;
	}
        //driver.print( std::cout ) << "\n";
    }
    else
    {
        // exit with failure condition
        return ( EXIT_FAILURE );
    }
    return( EXIT_SUCCESS );
*/
}
