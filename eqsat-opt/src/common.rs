use egg::*;

use std::usize::MAX;
use std::f64::INFINITY;
use std::cmp::Ordering;

define_language! {
    pub enum Prop {
        Bool(bool),
        Int(u32),
        "*" = And([Id; 2]),
        "!" = Not(Id),
        "+" = Or([Id; 2]),
        "^" = Xor([Id; 2]),
        // used for having multiple outputs
        "$" = Concat(Vec<Id>),
        "&" = Concat2([Id; 2]),
        Symbol(Symbol),
    }
}

pub enum PropId {
    And,
    Not,
    Xor,
    Lit,
    Sym
}

#[derive(Clone,Debug)]
pub struct DepthArea {
    pub depth: usize,
    pub area: f64,
}
impl DepthArea {
    pub fn cost(&self) -> usize {
        self.depth*self.depth * (self.area as usize)
    }
    pub fn new() -> Self {
        DepthArea { depth: 0, area: 0.0 }
    }
    pub fn max() -> Self {
        DepthArea { depth: MAX, area: INFINITY }
    }
}
impl std::ops::Add<DepthArea> for DepthArea {
    type Output = DepthArea;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            depth: std::cmp::max(self.depth,rhs.depth),
            area: self.area + rhs.area
        }
    }
}
impl PartialEq for DepthArea {
    fn eq(&self, other: &DepthArea) -> bool {
        self.depth == other.depth && self.area == other.area
    }
}
impl PartialOrd for DepthArea {
    fn partial_cmp(&self, other: &DepthArea) -> Option<Ordering> {
        if self.depth == other.depth {
            self.area.partial_cmp(&other.area)
        } else {
            self.depth.partial_cmp(&other.depth)
        }
    }
}
