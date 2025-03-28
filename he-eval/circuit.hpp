#pragma once

#include <string>
#include <vector>
#include <stdint.h>

class GateInp
{   
    public:
        bool is_gate;
        enum InpType
        {
            Const,
            Var
        };
        GateInp();
        InpType type;
        std::string name;
        bool polarity;
};

class Gate
{
    public:
        bool is_gate;
        enum Op
        {
            AND,
            XOR,
            OR,
            WIRE,
            UNSAFE_OR
        };
        Gate();


        Op op;
        GateInp* left;
        GateInp* right;
};