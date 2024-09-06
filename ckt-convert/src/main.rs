use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod parse;
mod eqn;
mod rules;

/// Simple program to greet a person
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Input file to operate on
    infile: PathBuf,
    /// Output file
    outfile: PathBuf,
    
}

#[derive(Subcommand)]
enum Commands {
    ConvertRules {
        #[arg(short, long, value_name = "CNT")]
        rulecnt: Option<u32>
    },
    ConvertEqn {
        #[arg(short, long, value_name = "NODE")]
        outnode: Option<String>
    },
}



fn main() {
    let args = Args::parse();

    match args.command {
        Commands::ConvertRules { rulecnt } => {
            rules::convert_rules(args.infile, args.outfile, rulecnt.map_or(-1, |r| (r as i32)));
        }
        Commands::ConvertEqn { outnode } => {
            eqn::convert_eqn(args.infile, args.outfile, outnode.as_deref());
        }
    }
}
