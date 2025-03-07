use crate::parse::{Xag,XagOp};
use crate::{eqn, parse};
use std::collections::HashMap;
use std::ops::Add;
use std::{cmp::max, path::PathBuf};

fn tree_mult_depth(x: &Xag, leaf_handle: &dyn Fn(&str) -> u32) -> u32 {
    match x.op.as_ref() {
        XagOp::Concat(ns) => ns.iter().map(|x: &Xag| tree_mult_depth(x, leaf_handle)).max().unwrap(),
        XagOp::Xor(n1, n2) => max(tree_mult_depth(n1, leaf_handle), tree_mult_depth(n2, leaf_handle)),
        XagOp::And(n1, n2) => 1 + max(tree_mult_depth(n1, leaf_handle), tree_mult_depth(n2, leaf_handle)),
        XagOp::Ident(i) => leaf_handle(&i),
        _ => 0,
    }
}

struct XagCount {
    mc: u32,
    xc: u32
} 

impl XagCount {
    fn zero() -> Self {
        XagCount {
            mc: 0,
            xc: 0
        }
    }
}

impl Add<XagCount> for XagCount {
    type Output = XagCount;
    fn add(self, rhs: XagCount) -> Self::Output {
        XagCount {
            mc: self.mc + rhs.mc,
            xc: self.xc + rhs.xc
        }
    }
}

fn tree_complexity(x: &Xag) -> XagCount {
    match x.op.as_ref() {
        XagOp::Concat(ns) => ns.iter().map(|x: &Xag| tree_complexity(x)).fold(XagCount::zero(), |acc, x| acc + x),
        XagOp::Xor(n1, n2) => {
            tree_complexity(n1) + tree_complexity(n2) + XagCount { mc: 0, xc: 1 }
        }
        XagOp::And(n1, n2) => {
            tree_complexity(n1) + tree_complexity(n2) + XagCount { mc: 1, xc: 0 }
        } 
        _ => XagCount::zero(),
    }
}

fn sexpr_stats(infile: PathBuf) {
    // open inrules and convert it to a vector of lines
    let sexpr = std::fs::read_to_string(infile).unwrap();
    let mut sexpr_lines = sexpr.lines();
    sexpr_lines.next();
    sexpr_lines.next();
    let sexpr = parse::lex(sexpr_lines.next().unwrap());
    let xag = parse::sexpr_to_xag(sexpr);
    let counts = tree_complexity(&xag);
    print!("{},{},{}", tree_mult_depth(&xag, &|_| {0}), counts.mc, counts.xc);
}

fn eqn_stats(infile: PathBuf) {
    let lines = std::fs::read_to_string(infile).unwrap();
    let eqn = eqn::parse_eqn(&lines);
    let mut depth: HashMap<String,u32> = HashMap::new();
    let mut mc: u32 = 0;
    let mut xc: u32 = 0;
    let mut md: u32 = 0;
    for net in eqn.lhses {
        let xag = eqn.equations.get(&net).unwrap();
        let x_md = tree_mult_depth(&xag, &|s| { *depth.get(s).unwrap_or(&0) });
        if x_md > md {
            md = x_md;
        }
        let counts = tree_complexity(&xag);
        mc += counts.mc;
        xc += counts.xc;
        depth.insert(net, x_md);
    }
    print!("{},{},{}", md, mc, xc);
}

pub fn file_stats(infile: PathBuf) {
    let path_str = infile.to_str().unwrap();
    if path_str.ends_with("eqn") {
        eqn_stats(infile);
    } else if path_str.ends_with("sexpr") {
        sexpr_stats(infile);
    } else {
        panic!("Unrecognized file extension: {}", path_str);
    }
}