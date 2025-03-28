#include "circuit.hpp"
#include <iostream>
#include <sstream>

using std::cout;
using std::endl;

GateInp::GateInp() 
{
    is_gate = false;
    polarity = true;
    name = "";
}

Gate::Gate()
{
    is_gate = true;
	left = NULL;
	right = NULL;
}