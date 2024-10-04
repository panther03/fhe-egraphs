#include "circuit.hpp"
#include <iostream>
#include <sstream>

using std::cout;
using std::endl;

GateInp::GateInp() 
{
    polarity = true;
    name = "";
}

Gate::Gate()
{
	left = NULL;
	right = NULL;
}