#pragma once

#include <string>
#include <cstddef>
#include <istream>
#include <vector>
#include <tuple>

#include "circuit.hpp"
#include "eqn_scanner.hpp"
#include "eqn_parser.tab.hh"

using namespace std;

namespace eqn
{
	class EqnDriver
	{
		public:
			EqnDriver() = default; 
			virtual ~EqnDriver();

			/** 
			 * parse - parse from a file
			 * @param filename - valid string with input file
			 */
			void parse( const char * const filename );
			/** 
			 * parse - parse from a c++ input stream
			 * @param is - std::istream&, valid input stream
			 */
			void parse( istream &iss );

			void add_word( const string &word );

			void add_input( const string &var );
			void add_output( const string &var );
			void add_eqn( pair<string, Gate*> &eqn );
			
			void add_eqn( const string &var );
			void add_consta();

			ostream& print(ostream &stream);

			vector<string> inputlist;
			vector<string> outputlist;
			vector<pair<string, Gate*>> eqnlist;
			vector<Gate> gatelist;
		private:

			void parse_helper( istream &stream );

			EqnParser  *parser  = nullptr;
			EqnScanner *scanner = nullptr;
	};

} /* end namespace EQN */