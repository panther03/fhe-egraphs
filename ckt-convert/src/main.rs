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
    ConvertEqn {
        #[arg(short, long, value_name = "NODE")]
        outnode: Option<String>,
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
        Commands::ConvertEqn { outnode, infile, outfile } => {
            eqn::convert_eqn(infile, outfile, outnode.as_deref());
        }
        Commands::Stats { infile } => { stats::file_stats(infile); },
        Commands::ConvertSexpr { infile, outfile } => {
            eqn::convert_sexpr(infile, outfile);
        }
    }
}