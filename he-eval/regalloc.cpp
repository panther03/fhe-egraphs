#include "regalloc.hpp"

using namespace std;

RegisterAllocator::RegisterAllocator (vector<string> &inputlist, vector<string> &outputlist, vector<pair<string, Gate*>> &eqnlist, bool quiet) {
    for (auto &input: inputlist) {
        regmap.insert(make_pair(input, id_counter++));
    }
    int inp_id_cnt = id_counter;

    unordered_set<string> out_nets;
    unordered_set<int> out_ids;
    for (auto &output: outputlist) {
        out_nets.insert(output);
    }

    // 1st pass: starting intervals
    for (auto &eqn : eqnlist) {
        string net = get<0>(eqn);
        int net_id = id_counter++;
        //cout << net_id << " goes to " << net << endl;
        regmap.insert(make_pair(net, net_id));
        if (out_nets.find(net) != out_nets.end()) {
            out_ids.insert(net_id);
        }
    }

    interval_ends = new int[id_counter];
    for (int i = 0; i < id_counter; i++) {
        // value needs to be alive until end of program
        // Should only be the case for output nodes, unless the input is malformed
        interval_ends[i] = id_counter;
    }

    // 2nd pass in reverse
    // collect interval ends 
    int id = id_counter - 1;
    for (auto riter = eqnlist.rbegin(); riter != eqnlist.rend(); ++riter, id--) {
        string net = get<0>(*riter);
        Gate* gate = get<1>(*riter);

        vector<int> children;
        switch (gate->op) {
            case Gate::Op::OR:
            case Gate::Op::XOR:
            case Gate::Op::AND: {
                GateInp* left = gate->left;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(left->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                GateInp* right = gate->right;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(right->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                break;
            }
            case Gate::Op::WIRE: {
                GateInp* left = gate->left;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(left->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                break;
            }
        }
        for (int child: children) {
            assert(child < id_counter);
            assert(regmap.find(net)->second == id);
            // Have not found the end yet
            if (interval_ends[child] == id_counter && out_ids.find(child) == out_ids.end()) {
                interval_ends[child] = id;
            }
        }
    }

    // Build interference graph
    for (int a_id = 0; a_id < id_counter; a_id++) {
        vector<int> a_neighbors;
        for (int b_id = 0; b_id < id_counter; b_id++) {
            if (a_id == b_id) { continue; }
            int last_start = max(a_id,b_id);
            int first_end = min(interval_ends[a_id],interval_ends[b_id]);
            // do the intervals overlap?
            // reverse of disjoint test (earliest finish is before last start)
            if (first_end > last_start) {
                //cout << a_id << " with " << b_id << endl;
                a_neighbors.push_back(b_id);
            }
        }
        neighbors[a_id] = a_neighbors;
    }

    // Initialize register allocation map
    allocation = new int[id_counter];
    for (int i = 0; i < id_counter; i++) {
        allocation[i] = -1;
    }

    // Perform greedy register allocation
    for (int r_id = 0; r_id < id_counter; r_id++) {
        vector<int>& r_neighbors = neighbors[r_id];
        int color = 0;
        unordered_set<int> neighbor_colors;
        for (int neighbor: r_neighbors) {
            assert(neighbor < id_counter);
            neighbor_colors.insert(allocation[neighbor]);
        }
        while (neighbor_colors.find(color) != neighbor_colors.end()) {
            color++;
        }
        allocation[r_id] = color;
        colors = max(colors, color+1);
        //cout << "Allocated color " << color << " for register " << r_id << endl;
        //cout << "Neighbors: " << endl; 
        //for (int neighbor: r_neighbors) {
        //    cout << neighbor << endl;
        //}
    }
    if (!quiet) cout << "(Colors: " << colors << ") ";

    return;
    
    /*
    // validation
    unordered_set<int> defined_regs;
    for (int i = 0; i < inp_id_cnt; i++) {
        defined_regs.insert(i);
    }
    for (auto & eqn: eqnlist) {
        string net = get<0>(eqn);
        int net_reg = net2reg(net);
        Gate* gate = get<1>(eqn);

        vector<int> children;
        switch (gate->op) {
            case Gate::Op::OR:
            case Gate::Op::XOR:
            case Gate::Op::AND: {
                GateInp* left = gate->left;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(left->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                GateInp* right = gate->right;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(right->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                break;
            }
            case Gate::Op::WIRE: {
                GateInp* left = gate->left;
                if (left->type == GateInp::InpType::Var) {
                    auto it = regmap.find(left->name);
                    assert(it != regmap.end());
                    children.push_back(it->second);
                }
                break;
            }
        }
        for (int child: children) {
            cout << net << " or " << net_reg << "needs " << allocation[child] << endl;
            assert(defined_regs.find(allocation[child]) != defined_regs.end());
        }
        defined_regs.insert(net_reg);
    }*/
}

int RegisterAllocator::net2reg(const string &net) const {
    auto it = regmap.find(net);
    assert(it != regmap.end());
    int r_id = it->second;
    assert(r_id < id_counter);
    return allocation[r_id];
}