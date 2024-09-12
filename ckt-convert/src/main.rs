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
    SexprStats {
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
        Commands::SexprStats { infile } => {
            // open inrules and convert it to a vector of lines
            let sexpr = std::fs::read_to_string(infile).unwrap();
            let mut sexpr_lines = sexpr.lines();
            sexpr_lines.next();
            sexpr_lines.next();
            let sexpr = parse::lex(sexpr_lines.next().unwrap());
            let xag = parse::sexpr_to_xag(sexpr);
            print!("{},{},", stats::mult_complexity(&xag), stats::mult_depth(&xag));
            //println!("MD: {}", stats::mult_depth(&xag));
        },
        Commands::ConvertSexpr { infile, outfile } => {
            eqn::convert_sexpr(infile, outfile);
        }
    }
}