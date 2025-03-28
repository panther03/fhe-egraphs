%skeleton "lalr1.cc"
%require  "3.0"
%debug 
%defines 
%define api.value.type variant
%define parse.assert
%define api.namespace {eqn}
%define api.parser.class {EqnParser}

%code requires{

	#include "circuit.hpp"
    #include <tuple>

	namespace eqn {
		class EqnDriver;
		class EqnScanner;
	}

// The following definitions is missing when %locations isn't used
# ifndef YY_NULLPTR
#  if defined __cplusplus && 201103L <= __cplusplus
#   define YY_NULLPTR nullptr
#  else
#   define YY_NULLPTR 0
#  endif
# endif

}

%parse-param { EqnScanner &scanner  }
%parse-param { EqnDriver &driver  }

%code{
   #include <iostream>
   #include <cstdlib>
   #include <fstream>
   
   /* include for all driver functions */
   #include "circuit.hpp"
   #include "eqn_driver.hpp"

#undef yylex
#define yylex scanner.yylex
}

%define parse.error verbose

%token TK_EOF		0     "end of file"
%token TK_INPUT_LIST
%token TK_OUTPUT_LIST
%token TK_LPAREN
%token TK_RPAREN
%token TK_EQUAL
%token TK_SEMICOLON
%token<int> TK_CONST_BOOL
%token<std::string> TK_VAR
%token TK_XOR
%token TK_OR
%token TK_AND
%token TK_NOT

%type<GateInp*> gateinp
%type<Gate*> gate
%type<std::pair<std::string, Gate*>> eqn

%right TK_AND
%right TK_OR

%locations

%%
main:
	TK_INPUT_LIST TK_EQUAL inputlist TK_SEMICOLON TK_OUTPUT_LIST TK_EQUAL outputlist TK_SEMICOLON eqnlist TK_EOF
;

inputlist
		: TK_VAR inputlist { driver.add_input( $1 ); }
		| TK_VAR { driver.add_input( $1 ); }
;

outputlist
		: TK_VAR outputlist { driver.add_output( $1 ); }
		| TK_VAR { driver.add_output( $1 ); }
;

eqnlist:
	   eqn eqnlist { driver.add_eqn( $1 ); }
	 | eqn { driver.add_eqn( $1 ); }
;

eqn:
   TK_VAR TK_EQUAL gate TK_SEMICOLON { $$ = std::make_pair($1, $3); }
;

gateinp
	: TK_VAR {
		GateInp *gi = new GateInp();
	
		gi->type = GateInp::InpType::Var;
		gi->name = $1;

		$$ = gi;
	}
	| TK_CONST_BOOL {
		GateInp *gi = new GateInp();
		gi->type = GateInp::InpType::Const;
		gi->polarity = ($1 != 0);

		$$ = gi;
	}
	| TK_NOT gateinp {
		GateInp *gi = $2;
		gi->polarity = !gi->polarity;
		$$ = gi;
	}

gate
	// expanded XOR rule
	/*: TK_LPAREN TK_NOT TK_VAR TK_AND TK_VAR TK_RPAREN TK_OR TK_LPAREN TK_VAR TK_AND TK_NOT TK_VAR TK_RPAREN 
		{
		Gate *g = new Gate();

		GateInp *l = new GateInp();
		l->type = GateInp::InpType::Var;
		l->name = $3;
		GateInp *r = new GateInp();
		r->type = GateInp::InpType::Var;
		r->name = $5;

		g->op = Gate::Op::XOR;
		g->left = l;
		g->right = r;

		$$ = g;
		}
	| TK_LPAREN TK_VAR TK_AND TK_NOT TK_VAR TK_RPAREN TK_OR TK_LPAREN TK_NOT TK_VAR TK_AND TK_VAR TK_RPAREN 
		{
		Gate *g = new Gate();

		GateInp *l = new GateInp();
		l->type = GateInp::InpType::Var;
		l->name = $2;
		GateInp *r = new GateInp();
		r->type = GateInp::InpType::Var;
		r->name = $5;

		g->op = Gate::Op::XOR;
		g->left = l;
		g->right = r;

		$$ = g;
		}*/
	: TK_LPAREN gate TK_RPAREN 
		{
		$$ = $2;
		}
	| gateinp TK_AND gateinp
		{
		Gate *g = new Gate();
		g->op = Gate::Op::AND;

		//GateInp *l = new GateInp();
		//l->type = GateInp::InpType::Var;
		//l->name = $1;
		//GateInp *r = new GateInp();
		//r->type = GateInp::InpType::Var;
		//r->name = $3;
		
		g->left = $1;
		g->right = $3;
		
		$$ = g;
		}	
	| gateinp TK_XOR gateinp
		{
		Gate *g = new Gate();
		g->op = Gate::Op::XOR;
		
		g->left = $1;
		g->right = $3;
		
		$$ = g;
		}	
	| gate TK_OR gate
		{
		Gate *g = new Gate();
		g->op = Gate::Op::UNSAFE_OR;
		
		// weird pointer trickery
		g->left = (GateInp*)$1;
		g->right = (GateInp*)$3;
		
		$$ = g;
		}
	| gateinp
		{
		Gate *g = new Gate();
		g->op = Gate::Op::WIRE;
		g->left = $1;

		$$ = g;
		}

%%


void 
eqn::EqnParser::error( const location_type &l, const std::string &err_message )
{
   std::cerr << "Error: " << err_message << " at " << l << "\n";
}
