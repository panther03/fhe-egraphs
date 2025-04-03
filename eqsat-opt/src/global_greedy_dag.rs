use std::cmp::Ordering;
use std::{iter, usize::MAX};
use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use ordered_float::NotNan;
use rpds::HashTrieSet;

use egraph_serialize::{ClassId,NodeId,Node,EGraph};
use crate::extraction_ser::{ExtractionResult};

#[derive(Debug, Clone, Copy)]
struct Cost {
    pub depth: egraph_serialize::Cost,
    pub area: egraph_serialize::Cost,
    pub depth_bound: egraph_serialize::Cost
}
pub const C_INFINITY: egraph_serialize::Cost = unsafe { NotNan::new_unchecked(std::f64::INFINITY) };
pub const C_ZERO: egraph_serialize::Cost = unsafe { NotNan::new_unchecked(0.0) };

impl Cost {
    pub const fn INFINITY(bound: egraph_serialize::Cost) -> Self {
        Self { depth: C_INFINITY, area: C_INFINITY, depth_bound: bound }
    }
    pub const ZERO: Self = Self { depth: C_ZERO, area: C_ZERO, depth_bound: C_ZERO };

    fn add_node_cost(self, c: egraph_serialize::Cost) -> Self {
        Self {
            depth: self.depth + c,
            area: self.area + c,
            depth_bound: self.depth_bound
        }
    }

    fn set_bound(self, bound: egraph_serialize::Cost) -> Self {
        Self {
            depth: self.depth,
            area: self.area,
            depth_bound: bound
        }
    }
}

impl std::ops::Add for Cost {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self { 
            depth: self.depth.max(rhs.depth),
            area: self.area + rhs.area,
            depth_bound: self.depth_bound
        }
    }
}

impl std::ops::AddAssign for Cost {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}


impl PartialEq for Cost {
    fn eq(&self, other: &Cost) -> bool {
        self.depth == other.depth && self.area == other.area && self.depth_bound == other.depth_bound
    }
}

impl PartialOrd for Cost {
    // we can change this.
    // instead of saying if self.depth == other.depth,
    // say let d1_sat = (self.depth + self.inv_md > ckt_md) as usize;
    // say let d2_sat = (other.depth + other.inv_md > ckt_md) as usize;
    // if d1_sat == d2_sat {
    //  compare areas
    // } else { compare d1_sat, d2_sat }
    // the minimum one will be the one that satisfies this property.
    // 
    // why would this work here and not in filtration for computing the bounds?
    // because for this, a filtered node is one that we know is too expensive.
    // while in the bounds computation, a filtered node is one that we don't care about.
    // suppose there is an e-node with one child that is filtered and one that has md 3.
    // here, the cost of that node is infinity, while in the bounds step the cost is 3.
    // because having the info that a node is filtered can make a class either more or less expensive, more legwork is required.
    // it's not possible to define a partial order like this.
    fn partial_cmp(&self, other: &Cost) -> Option<Ordering> {
        assert!(self.depth_bound == other.depth_bound);
        let d1_unsat = (self.depth > self.depth_bound) as usize;
        let d2_unsat = (other.depth > other.depth_bound) as usize;
        if d1_unsat == d2_unsat {
            self.area.partial_cmp(&other.area)
        } else {
            self.depth.partial_cmp(&other.depth)
        }
    }
}

type TermId = usize;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Term {
    op: String,
    children: Vec<TermId>,
}

type Reachable = HashTrieSet<ClassId>;

#[derive(Debug)]
struct TermInfo {
    node: NodeId,
    eclass: ClassId,
    node_cost: egraph_serialize::Cost,
    total_cost: Cost,
    // store the set of reachable terms from this term
    reachable: Reachable,
    size: usize,
}

/// A TermDag needs to store terms that share common
/// subterms using a hashmap.
/// However, it also critically needs to be able to answer
/// reachability queries in this dag `reachable`.
/// This prevents double-counting costs when
/// computing the cost of a term.
#[derive(Default)]
pub struct TermDag {
    nodes: Vec<Term>,
    info: Vec<TermInfo>,
    hash_cons: HashMap<Term, TermId>,
}

impl TermDag {
    /// Makes a new term using a node and children terms
    /// Correctly computes total_cost with sharing
    /// If this term contains itself, returns None
    /// If this term costs more than target, returns None
    pub fn make(
        &mut self,
        node_id: NodeId,
        node: &Node,
        children: Vec<TermId>,
        bounds: &HashMap<ClassId, i32>,
        target: Cost,
    ) -> Option<TermId> {
        let term = Term {
            op: node.op.clone(),
            children: children.clone(),
        };

        if let Some(id) = self.hash_cons.get(&term) {
            return Some(*id);
        }

        let bound = bounds.get(&node.eclass).unwrap_or_else(|| {
            println!("could not get bounds info for {}", node.eclass);
            &0});
        let bound = unsafe {
            NotNan::new_unchecked(*bound as f64)
        };

        if children.is_empty() {
            let next_id = self.nodes.len();
            let node_cost = Cost::ZERO.add_node_cost(node.cost).set_bound(bound);
            self.nodes.push(term.clone());
            self.info.push(TermInfo {
                node: node_id,
                eclass: node.eclass.clone(),
                node_cost: node.cost,
                total_cost: node_cost,
                reachable: iter::once(node.eclass.clone()).collect(),
                size: 1,
            });
            self.hash_cons.insert(term, next_id);
            Some(next_id)
        } else {
            // check if children contains this node, preventing cycles
            // This is sound because `reachable` is the set of reachable eclasses
            // from this term.
            for child in &children {
                if self.info[*child].reachable.contains(&node.eclass) {
                    return None;
                }
            }

            let biggest_child = (0..children.len())
                .max_by_key(|i| self.info[children[*i]].size)
                .unwrap();

            let mut cost = self.total_cost(children[biggest_child]);
            let mut reachable = self.info[children[biggest_child]].reachable.clone();
            let next_id = self.nodes.len();
            
            for child in children.iter() {
                if cost.add_node_cost(node.cost).set_bound(bound) > target {
                    return None;
                }
                let child_cost = self.get_cost(&mut reachable, *child);
                cost += child_cost;
            }
            cost = cost.add_node_cost(node.cost).set_bound(bound);

            if cost > target {
                return None;
            }

            reachable = reachable.insert(node.eclass.clone());

            self.info.push(TermInfo {
                node: node_id,
                node_cost: node.cost,
                eclass: node.eclass.clone(),
                total_cost: cost,
                reachable,
                size: 1 + children.iter().map(|c| self.info[*c].size).sum::<usize>(),
            });
            self.nodes.push(term.clone());
            self.hash_cons.insert(term, next_id);
            Some(next_id)
        }
    }

    /// Return a new term, like this one but making use of shared terms.
    /// Also return the cost of the new nodes.
    fn get_cost(&self, shared: &mut Reachable, id: TermId) -> Cost {
        let eclass = self.info[id].eclass.clone();

        // This is the key to why this algorithm is faster than greedy_dag.
        // While doing the set union between reachable sets, we can stop early
        // if we find a shared term.
        // Since the term with `id` is shared, the reachable set of `id` will already
        // be in `shared`.
        if shared.contains(&eclass) {
            Cost {depth: self.info[id].total_cost.depth, area: C_ZERO, depth_bound: C_ZERO}
        } else {
            let mut cost = Cost::ZERO;
            for child in &self.nodes[id].children {
                let child_cost = self.get_cost(shared, *child);
                cost += child_cost;
            }
            cost = cost.add_node_cost(self.node_cost(id));
            *shared = shared.insert(eclass);
            cost
        }
    }

    pub fn node_cost(&self, id: TermId) -> egraph_serialize::Cost {
        self.info[id].node_cost
    }

    pub fn total_cost(&self, id: TermId) -> Cost {
        self.info[id].total_cost
    }
}

pub fn mc_extract(egraph: &EGraph, roots: &[ClassId], locked: HashMap<ClassId, NodeId>, bounds: &HashMap<ClassId, i32>) -> ExtractionResult {
    let mut keep_going = true;

    let nodes = egraph.nodes.clone();
    let mut termdag = TermDag::default();
    let mut best_in_class: HashMap<ClassId, TermId> = HashMap::default();

    while keep_going {
        keep_going = false;

        'node_loop: for (node_id, node) in &nodes {
            match locked.get(&node.eclass) {
                Some(locked_node) if node_id != locked_node => { continue; }
                _ => {}
            }
            let mut children: Vec<TermId> = vec![];
            // compute the cost set from the children
            for child in &node.children {
                let child_cid = egraph.nid_to_cid(child);
                if let Some(best) = best_in_class.get(child_cid) {
                    children.push(*best);
                } else {
                    continue 'node_loop;
                }
            }

            let bound = bounds.get(&node.eclass).unwrap_or_else(|| {
                println!("could not get bounds info for {}", node.eclass);
                &0});
            let bound = unsafe {
                NotNan::new_unchecked(*bound as f64)
            };
            let old_cost = best_in_class
                .get(&node.eclass)
                .map(|id| termdag.total_cost(*id))
                .unwrap_or(Cost::INFINITY(bound));

            if let Some(candidate) = termdag.make(node_id.clone(), node, children, bounds, old_cost) {
                let cadidate_cost = termdag.total_cost(candidate);

                if cadidate_cost < old_cost {
                    best_in_class.insert(node.eclass.clone(), candidate);
                    keep_going = true;
                }
            }
        }
    }

    // ????
    //let mut node_to_cost: HashMap<NodeId, f64> = HashMap::new();
    //for (node_id, node) in &nodes {
    //    let cost = best_in_class.get(&node.eclass).unwrap_or(&MAX);
    //    node_to_cost.insert(node_id.clone(), *cost as f64);
    //}
    //node_to_cost

    let mut result: ExtractionResult = IndexMap::new();
    for (class, term) in best_in_class {
        let info = &termdag.info[term];
        result.insert(class, (info.total_cost.depth.round() as usize, info.node));
    }
    result
}