use std::{collections::{HashMap, HashSet}, ops::{Add, AddAssign}};
use egraph_serialize::*;


pub fn should_visit_complete_class(visited: &HashSet<ClassId>, egraph: &EGraph, class: &Class) -> bool {
    if visited.contains(&class.id) {
        return false;
    }

    class.nodes.iter().all(|n|  {
        egraph[n].children.iter().all(|c| visited.contains(&ClassId::new(c.class())))
    })
}

// not using this atm going with just copy paste
pub trait CostVal<T: Ord + Add + Clone + Sized = Self>: Ord + Add<Output=Self> + Clone + Copy + Sized {
    fn unvisited() -> Self;
    fn zero() -> Self;
    fn from_egraph_cost(cost: egraph_serialize::Cost) -> Self;
}

pub fn should_visit_cost_depth<C: CostVal>(costs: &HashMap<ClassId, C>, egraph: &EGraph, class: &Class) -> bool {
    let unv = C::unvisited();
    let class_cost = costs.get(&class.id).unwrap_or(&unv);
    class.nodes.iter().any(|n| {
        let cost = (&egraph[n]).cost;
        let cost = C::from_egraph_cost(cost);
        let mut node_cost = C::zero();
        egraph[n].children.iter().for_each(|c| {
            let c = &ClassId::new(c.class());
            let child_cost = (costs.get(c).unwrap_or(&C::unvisited())).clone();
            node_cost = std::cmp::max(node_cost, child_cost);
        });
        (cost + node_cost) < *class_cost
    })
}

pub trait EGraphVisitor {
    fn should_visit(&mut self, egraph: &EGraph, class: &Class) -> bool;

    fn visit(&mut self, egraph: &EGraph, class: &Class);
}

pub fn egraph_pass_traverse<V: EGraphVisitor> (
    egraph: &EGraph,
    visitor: &mut V
) -> usize  {
    let mut did_something = true;
    let mut iters = 0;
    while did_something {
        did_something = false;
        for (_, class) in egraph.classes() {
            if visitor.should_visit(egraph, class) {
                visitor.visit(egraph, class);
                did_something = true;
            }
        }
        iters += 1;
    }
    iters
}