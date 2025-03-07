use egraph_serialize::*;
use std::{fs::File, path::PathBuf};
use std::io::Write;
use std::fmt::Write as OtherWrite;

pub fn egraph2dot(infile: PathBuf, outfile: PathBuf) -> Result<(), std::io::Error> {
    let egraph = EGraph::from_json_file(infile).expect("Failed to parse egraph: {}");
    let mut outbuf = File::create(outfile).unwrap();

    let mut connections = String::new();

    write!(outbuf, "digraph EGraph {{\nrankdir=TB;\ncompound=true;\nnewrank=true\n")?;
    for (cid, c) in egraph.classes() {
        write!(outbuf, "\tsubgraph cluster_eclass{} {{\n", cid)?;
        for nid in c.nodes.iter() {
            let node = egraph.nodes.get(nid).unwrap();
            let (op_sym, fsize) = match node.op.as_str() {
                "*" => ("∧", 14),
                "^" => ("⊕", 18),
                _ => (node.op.as_str(), 16)
            };
            write!(outbuf, "\t\t{} [label=\"{}\" fontsize=\"{}\"];\n", nid, op_sym, fsize)?;
            for child in node.children.iter() {
                let c_node = egraph.nodes.get(child).unwrap();
                write!(connections, "\t\"{}\" -> \"{}\"  [lhead=cluster_eclass{}];\n", nid, child, c_node.eclass).unwrap();
            }
        }
        write!(connections,"\n").unwrap();
        write!(outbuf,"\t}}\n")?;
    }
    write!(outbuf, "{}", connections)?;
    write!(outbuf,"}}\n")?;
    Ok(())
}