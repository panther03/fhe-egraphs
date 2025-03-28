use std::ops::Index;
use std::{iter, usize::MAX};
use std::collections::{HashMap, HashSet};
    use crate::extraction_ser::ExtractionResult;

    use indexmap::IndexMap;
use ordered_float::{Float, NotNan};
use rpds::HashTrieSet;


use egraph_serialize::{ClassId,NodeId,Node,Cost,EGraph};

pub const INFINITY: Cost = unsafe { NotNan::new_unchecked(std::f64::INFINITY) };

type TermId = usize;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Term {
    //op: String,
    children: Vec<TermId>,
}

type Reachable = HashTrieSet<ClassId>;

#[allow(unused)]
struct TermInfo {
    node: NodeId,
    eclass: ClassId,
    node_cost: Cost,
    total_cost: Cost,
    total_depth: usize,
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
        target: Cost,
        data: &MdMcExtractData
    ) -> Option<TermId> {
        let term = Term {
            //op: node.op.clone(),
            children: children.clone(),
        };

        if let Some(id) = self.hash_cons.get(&term) {
            return Some(*id);
        }

        let node_cost = node.cost;

        if children.is_empty() {
            let next_id = self.nodes.len();
            self.nodes.push(term.clone());
            self.info.push(TermInfo {
                node: node_id,
                eclass: node.eclass.clone(),
                node_cost,
                total_cost: node_cost,
                total_depth: (node.cost.round()) as usize,
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
            let deepest_child = (0..children.len())
                .max_by_key(|i| self.info[children[*i]].total_depth)
                .unwrap();

            if deepest_child + data.inv_md_lookup.get(&node_id).unwrap() > data.ckt_md {
                return None;
            }

            let node_cost_u= node_cost.round() as usize;
            let mut cost = node_cost + self.total_cost(children[biggest_child]);
            let mut reachable = self.info[children[biggest_child]].reachable.clone();
            let next_id = self.nodes.len();

            for child in children.iter() {
                if cost > target {
                    return None;
                }
                let child_cost = self.get_cost(&mut reachable, *child);
                cost += child_cost;
            }

            if cost > target {
                return None;
            }

            reachable = reachable.insert(node.eclass.clone());

            self.info.push(TermInfo {
                node: node_id,
                node_cost,
                eclass: node.eclass.clone(),
                total_cost: cost,
                total_depth: deepest_child + node_cost_u,
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
            // should be (depth of eclass, 0.0) 
            ordered_float::NotNan::<f64>::new(0.0).unwrap()
        } else {
            let mut cost = self.node_cost(id);
            for child in &self.nodes[id].children {
                let child_cost = self.get_cost(shared, *child);
                cost += child_cost;
            }
            *shared = shared.insert(eclass);
            cost
        }
    }

    pub fn node_cost(&self, id: TermId) -> Cost {
        self.info[id].node_cost
    }

    pub fn total_cost(&self, id: TermId) -> Cost {
        self.info[id].total_cost
    }
}

pub fn mc_extract(egraph: &EGraph, _roots: &[ClassId]) -> ExtractionResult {
    let mut keep_going = true;

    let nodes = egraph.nodes.clone();
    let mut termdag = TermDag::default();
    let mut best_in_class: HashMap<ClassId, TermId> = HashMap::default();

    let mut data = MdMcExtractData::new();
    data.md_extract(egraph, _roots);
    dbg!(data.ckt_md);

    while keep_going {
        keep_going = false;

        'node_loop: for (node_id, node) in &nodes {
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

            let old_cost = best_in_class
                .get(&node.eclass)
                .map(|id| termdag.total_cost(*id))
                .unwrap_or(INFINITY);

            if let Some(candidate) = termdag.make(node_id.clone(), node, children, old_cost, &data) {
                let cadidate_cost = termdag.total_cost(candidate);

                if cadidate_cost < old_cost {
                    best_in_class.insert(node.eclass.clone(), candidate);
                    keep_going = true;
                }
            }
        }
    }

    /*for (_, term) in best_in_class {
        node_to_cost.insert(termdag.info[term].node.clone(), termdag.total_cost(term).into());
    }*/
    let mut result: ExtractionResult = IndexMap::new();
    for (node_id, node) in &nodes {
        result.insert(node.eclass.clone(), (0, node_id.clone()));
        //let cost = best_in_class.get(&node.eclass).unwrap_or(&MAX);
        //node_to_cost.insert(node_id.clone(), *cost as f64);
    }
    result
}

pub struct MdMcExtractData {
    inv_md_lookup: HashMap<NodeId, usize>,
    md_lookup: HashMap<NodeId, usize>,
    ckt_md: usize
}

impl MdMcExtractData {
    pub fn new() -> Self {
        Self {
            inv_md_lookup: HashMap::new(),
            md_lookup: HashMap::new(),
            ckt_md: 0
        }
    }

    // this actually needs to use the md-extracted graph, NOT the e-graph
    // then measure the worst distance to any node
    pub fn md_extract(&mut self, egraph: &EGraph, _roots: &[ClassId]) {
        enum WorkItem<'a> {
            Child(usize, &'a ClassId),
            Continuation(&'a ClassId)   
        }

        let mut worklist: Vec<WorkItem> = Vec::new();
        let mut path: HashSet<ClassId> = HashSet::new();
        for root in _roots {    
            worklist.push(WorkItem::Child(0, root));
            path.insert(root.clone());
        }
    
        let mut worst_md = 0;
        
        dbg!("b");
        while !worklist.is_empty() {
            let item = worklist.pop().unwrap();
            match item {
                WorkItem::Child(inv_md, eclass) => {
                    worklist.push(WorkItem::Continuation(eclass));

                    let mut nodes: Vec<(&NodeId,usize)> = Vec::new();
                    for node in &egraph[eclass].nodes {
                        nodes.push((node, (&egraph[node]).cost.round() as usize));
                    }
                    nodes.sort_by(|a,b| a.1.cmp(&b.1));
                    let min_cost = nodes.iter().min_by_key(|x| x.1).map(|m| m.1);
                    
                    dbg!("sa");
                    for (node, cost) in nodes {
                        dbg!(cost);
                        /*if cost > min_cost.unwrap() {
                            continue;
                        }*/

                        let inv_md_n = inv_md + cost;
                        self.inv_md_lookup.insert(node.clone(), inv_md);
                        if inv_md_n > worst_md {
                            worst_md = inv_md_n;
                        }
                        for child in &egraph[node].children {
                            let child_ec = egraph.nid_to_cid(child);
                            let existing_inv_md = self.inv_md_lookup.get(child);
                            // cycle check
                            if !path.contains(child_ec) && (existing_inv_md.is_none() || inv_md_n < *existing_inv_md.unwrap()) {
                                worklist.push(WorkItem::Child(inv_md_n, child_ec));
                            }
                            path.insert(child_ec.clone());
                        }
                    }
                    dbg!("st");
                }
                WorkItem::Continuation(eclass) => {
                    path.remove(eclass);
                }
            }            
        }
        self.ckt_md = worst_md;
    }
/*
    pub fn md_extract(&mut self, egraph: &EGraph, _roots: &[ClassId]) {
        let mut worklist: Vec<(usize, &ClassId)> = Vec::new();
        for root in _roots {
            worklist.push((0, root));
        }
        let mut ckt_md = 0;
        while !worklist.is_empty() {
            let (p_inv_md, eclass) = worklist.pop().unwrap(); 

            let mut nodes: Vec<(&NodeId,usize)> = Vec::new();
            for node in &egraph[eclass].nodes {
                nodes.push((node, (&egraph[node]).cost.round() as usize));
            }
            nodes.sort_by(|a,b| a.1.cmp(&b.1));
            let min_cost = nodes.iter().min_by_key(|x| x.1).map(|m| m.1);           

            for node in &egraph[eclass].nodes {
                let inv_md = p_inv_md + (&egraph[node]).cost.round() as usize;
                if inv_md > ckt_md {
                    ckt_md = inv_md;
                }
                self.inv_md_lookup.insert(*node, p_inv_md);
                for child in &egraph[node].children {
                    let child_ec = egraph.nid_to_cid(child);
                    let existing_p_inv_md = self.inv_md_lookup.get(child);
                    // cycle check
                    if existing_p_inv_md.is_none() || inv_md < *existing_p_inv_md.unwrap() {
                        worklist.push((inv_md, child_ec));
                        self.inv_md_lookup.insert(*child, inv_md);
                    }
                }
            }
        }
        self.ckt_md = ckt_md;
    }*/
}
