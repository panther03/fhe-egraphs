use crate::parse::{Xag,XagOp};
use std::cmp::max;

pub fn mult_depth(x: &Xag) -> u32 {
    match x.op.as_ref() {
        XagOp::Concat(ns) => ns.iter().map(|x: &Xag| mult_depth(x)).max().unwrap(),
        XagOp::Xor(n1, n2) => max(mult_depth(n1), mult_depth(n2)),
        XagOp::And(n1, n2) => 1 + max(mult_depth(n1), mult_depth(n2)),
        _ => 0,
    }
}

pub fn mult_complexity(x: &Xag) -> u32 {
    match x.op.as_ref() {
        XagOp::Concat(ns) => ns.iter().map(|x: &Xag| mult_depth(x)).sum(),
        XagOp::Xor(n1, n2) => mult_complexity(n1) + mult_complexity(n2),
        XagOp::And(n1, n2) => 1 + mult_depth(n1) + mult_depth(n2),
        _ => 0,
    }
}