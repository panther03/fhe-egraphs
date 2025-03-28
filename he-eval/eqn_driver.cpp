#include <cctype>
#include <fstream>
#include <cassert>

#include "eqn_driver.hpp"
#include "circuit.hpp"

using namespace eqn;

EqnDriver::~EqnDriver()
{
   delete(scanner);
   scanner = nullptr;
   delete(parser);
   parser = nullptr;
}

void 
EqnDriver::parse( const char * const filename )
{
   assert( filename != nullptr );
   std::ifstream in_file( filename );
   if( ! in_file.good() )
   {
       exit( EXIT_FAILURE );
   }
   parse_helper( in_file );
   return;
}

void
EqnDriver::parse( std::istream &stream )
{
   if( ! stream.good()  && stream.eof() )
   {
       return;
   }
   //else
   parse_helper( stream ); 
   return;
}


void 
EqnDriver::parse_helper( std::istream &stream )
{
   
   delete(scanner);
   try
   {
      scanner = new EqnScanner( &stream );
   }
   catch( std::bad_alloc &ba )
   {
      std::cerr << "Failed to allocate scanner: (" <<
         ba.what() << "), exiting!!\n";
      exit( EXIT_FAILURE );
   }
   
   delete(parser); 
   try
   {
      parser = new EqnParser( (*scanner) /* scanner */, 
                                  (*this) /* driver */ );
   }
   catch( std::bad_alloc &ba )
   {
      std::cerr << "Failed to allocate parser: (" << 
         ba.what() << "), exiting!!\n";
      exit( EXIT_FAILURE );
   }
   const int accept( 0 );
   if( parser->parse() != accept )
   {
      std::cerr << "Parse failed!!\n";
      exit(EXIT_FAILURE);
   }
   return;
}


void
EqnDriver::add_input( const std::string &var )
{
	inputlist.push_back(var);
}

void
EqnDriver::add_output ( const std::string &var )
{
	outputlist.push_back(var);
}

void
EqnDriver::add_eqn( std::pair<std::string, Gate*> &eqn )
{
   Gate* g = eqn.second;
   if (g->op == Gate::UNSAFE_OR) {
      assert(g->left->is_gate && g->right->is_gate);
      Gate* a1 = (Gate*) g->left;
      Gate* a2 = (Gate*) g->right;
      if (a1->op == Gate::AND && a2->op == Gate::AND) {
         g->left = a1->left;
         g->left->polarity = false;
         g->right = a1->right;
         g->right->polarity = false;
         g->op = Gate::XOR;
      } else {
         assert(a1->op == Gate::WIRE && a2->op == Gate::WIRE);
         g->op = Gate::OR;
      }
   }
   eqnlist.insert(eqnlist.begin(), eqn);
	//eqnlist.push_back(eqn);
}


std::ostream& 
EqnDriver::print( std::ostream &stream )
{
   stream << "INPUT LIST:" << std::endl;
   for(vector<string>::size_type i=0 ; i<inputlist.size(); i++)
   {
	   cout << inputlist.at(i) << endl;
   }

   stream << "OUTPUT LIST:" << std::endl;
   for(vector<string>::size_type i=0 ; i<outputlist.size(); i++)
   {
	   cout << outputlist.at(i) << endl;
   }

   stream << "EQN2 LIST SIZE :" << eqnlist.size() << std::endl;
   
   /*for(vector<pair<string, MC::Bexp*>>::size_type i=0 ; i<eqnlist.size(); i++)
   {
	   MC::Bexp* b = get<1>(eqnlist.at(i));
	   cout << "VAR: " << get<0>(eqnlist.at(i)) << "  HEAD: " << get<1>(eqnlist.at(i))->head << endl;

	   if (b->left) {
		   cout << "LCHILD: " << b->left->var << "  ";
		   if (b->right) {
			   cout << "RCHILD: " << b->right->var << endl;
		   }
		   else {
			   cout << endl;
		   }
	   }
   }*/
   return(stream);
}
