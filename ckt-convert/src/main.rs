use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod parse;
mod eqn;
mod dot;
mod rules;
mod stats;

/// Convert various circuit formats.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name="lobster2egg-rules")]
    Lobster2EggRules {
        #[arg(short, long, value_name = "CNT")]
        rulecnt: Option<u32>,
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="cut-rewrite2egg-rules")]
    CutRewrite2EggRules {
        /// File containing lhs xag => truth table
        lhses: PathBuf,
        /// File containing truth table => rhs xag
        rhses: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="eqn2sexpr")]
    Eqn2Sexpr {
        #[arg(short, long, value_name = "NODE")]
        outnode: Option<String>,
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="eqn2seqn")]
    Eqn2Seqn {
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="eqn2egglog")]
    Eqn2Egglog {
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="sexpr2eqn")]
    Sexpr2Eqn {
        /// Input file to operate on
        infile: PathBuf,
        /// Output file
        outfile: PathBuf,
    },
    #[command(name="egraph2dot")]
    Egraph2Dot {
        infile: PathBuf,
        outfile: PathBuf
    },
    Stats {
        /// Input file to operate on
        infile: PathBuf,
    }
}


fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Lobster2EggRules { rulecnt, infile, outfile} => {
            rules::convert_rules(infile, outfile, rulecnt.map_or(-1, |r| (r as i32)));
        }
        Commands::CutRewrite2EggRules { lhses, rhses, outfile} => {
            rules::convert_cut_rewriting_rules(lhses, rhses, outfile);
        }
        Commands::Eqn2Sexpr { outnode, infile, outfile } => {
            eqn::eqn2sexpr(infile, outfile, outnode.as_deref());
        }
        Commands::Eqn2Seqn { infile, outfile } => {
            eqn::eqn2seqn(infile, outfile );
        }
        Commands::Eqn2Egglog { infile, outfile } => {
            eqn::eqn2egglog(infile, outfile );
        }
        Commands::Stats { infile } => { stats::file_stats(infile); },
        Commands::Sexpr2Eqn { infile, outfile } => {
            eqn::sexpr2eqn(infile, outfile);
        },
        Commands::Egraph2Dot { infile, outfile } => {
            dot::egraph2dot(infile, outfile).unwrap();
        }
    }
}