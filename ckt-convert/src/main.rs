use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod parse;
mod eqn;
mod rules;
mod stats;

use parse::Token;

/// Simple program to greet a person
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ConvertRules {
        #[arg(short, long, value_name = "CNT")]
        rulecnt: Option<u32>,
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    ConvertMcRules {
        /// File containing lhs xag => truth table
        lhses: PathBuf,
        /// File containing truth table => rhs xag
        rhses: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    ConvertEqn {
        #[arg(short, long, value_name = "NODE")]
        outnode: Option<String>,
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    ConvertSEqn {
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    ConvertSexpr {
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    Stats {
        /// Input file to operate on
        infile: PathBuf,
    }
}


fn main() {
    let args = Args::parse();

    match args.command {
        Commands::ConvertRules { rulecnt, infile, outfile} => {
            rules::convert_rules(infile, outfile, rulecnt.map_or(-1, |r| (r as i32)));
        }
        Commands::ConvertMcRules { lhses, rhses, outfile} => {
            rules::convert_cut_rewriting_rules(lhses, rhses, outfile);
        }
        Commands::ConvertEqn { outnode, infile, outfile } => {
            eqn::convert_eqn(infile, outfile, outnode.as_deref());
        }
        Commands::ConvertSEqn { infile, outfile } => {
            eqn::convert_seqn(infile, outfile );
        }
        Commands::Stats { infile } => { stats::file_stats(infile); },
        Commands::ConvertSexpr { infile, outfile } => {
            eqn::convert_sexpr(infile, outfile);
        }
    }
}