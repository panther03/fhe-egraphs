#include <map>
#include <string>
#include <unordered_set>
#include <vector>

#include "eqn_driver.hpp"

using namespace std;

class RegisterAllocator {
    private: 
        map<int, vector<int>> neighbors;
        map<string, int> regmap;
        int* interval_ends;
        int* allocation;
        int id_counter = 0;

        void update_neighbor (int child, int parent);
        void update_neighbor (int net, vector<int>& children);
    public:
        int colors = 0;

        RegisterAllocator (vector<string> &inputlist, vector<tuple<string, Gate*>> &eqnlist, bool quiet);
        int net2reg(const string &net) const;
};