#pragma once

#include <string>
#include <vector>
#include <stdint.h>

class GateInp
{
    public:
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
        enum Op
        {
            AND,
            XOR,
            OR,
            WIRE
        };
        Gate();

        Op op;
        
        GateInp* left;
        GateInp* right;
};