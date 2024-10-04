#pragma once

#if !defined(yyFlexLexerOnce)
#include <FlexLexer.h>
#endif

#include "eqn_parser.tab.hh"
#include "location.hh"

namespace eqn
{

    class EqnScanner : public yyFlexLexer
    {
    public:
        EqnScanner(std::istream *in) : yyFlexLexer(in) {
                                       };
        virtual ~EqnScanner() {
        };

        // get rid of override virtual function warning
        using FlexLexer::yylex;

        virtual int yylex(EqnParser::semantic_type *const lval,
                          EqnParser::location_type *location);
        // YY_DECL defined in eqn_lexer.l
        // Method body created by flex in eqn_lexer.yy.cc

    private:
        /* yyval ptr */
        EqnParser::semantic_type *yylval = nullptr;

    }; /* end namespace eqn  */
}