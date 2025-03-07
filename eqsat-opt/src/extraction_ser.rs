// Extraction algorithms working directly on
// serialized data structure from egraph-serialize.

use egg::Extractor;
use egraph_serialize::{ClassId,NodeId,EGraph};
use std::collections::{HashMap, HashSet};
use std::f64::INFINITY;
use std::ops::Index;
use std::usize::MAX;

use crate::common::{DepthArea, Prop, PropId};

use indexmap::{map::Entry, IndexMap};

type ExtractionResult = IndexMap<ClassId,(usize,NodeId)>;

pub struct MixedCost {
    pub enode_opt_lookup: HashMap<NodeId, f64>,
    pub results: ExtractionResult,
    pub visited: HashSet<ClassId>
}
impl MixedCost {
    pub fn select_best_eclass_mixed(&mut self, egraph: &EGraph, eclass: ClassId, depth: usize) -> usize {
        let mut best_cost: DepthArea = DepthArea::max();
        let eclass_s = &egraph[&eclass];
        // We need a default, because we might be in a case where the e-class has only one node which is a cycle.
        // In this case, the cost will never be lower, so the best enode is never updated.
        let mut best_enode: NodeId = eclass_s.nodes[0];
        if self.visited.get(&eclass).is_some() {
            // cycle detected, ignore this node
            return MAX;
        }
        self.visited.insert(eclass.clone());
        assert!(!eclass_s.nodes.is_empty());
        for nodeid in eclass_s.nodes.iter() {
            let node = &egraph[nodeid];
            let worst_depth:usize = node.children.iter().map(|c| {
                let c = egraph.nid_to_cid(c);                
                // filter out cycles, although these shouldn't ei
                self.results
                    .get(c)
                    .map(|x| { x.0})
                    .unwrap_or_else(|| self.select_best_eclass_mixed(egraph, c.clone(), depth+1))
            }).max().unwrap_or(0);
            if worst_depth == MAX {
                continue;
            }
            let dag_area = *self.enode_opt_lookup
                .get(&nodeid)
                .unwrap_or(&INFINITY);
            let md_cost = match node.op.as_str() {
                "*" => 1,
                _ => 0
            };
            let node_cost = DepthArea {
                area: dag_area,
                depth: worst_depth.saturating_add(md_cost)
            };
            // this makes a big difference in the quality of area results for some reason???
            if node_cost < best_cost {
                best_cost = node_cost;
                best_enode = nodeid.clone();
            }
        }
        self.results.insert(eclass, (best_cost.depth, best_enode));
        return best_cost.depth;
    }
}

pub fn dag_network_writer(
    egraph: &EGraph,
    cost_analysis: &mut ExtractionResult,
    out_net_to_eclass: &IndexMap<String, egg::Id>
) -> String {
    // temporary network to hold nodes whose children have not been visited yet
    let mut network: Vec<String> = Vec::new();
    // the network which we will output
    let mut real_network: Vec<String> = Vec::new();
    // set of visited nodes
    let mut eclass_seen: HashSet<ClassId> = HashSet::new();
    // new extraction result, in topological order, and only containing nodes visited
    let mut topo_cost_analysis: ExtractionResult = IndexMap::new();

    // stack of visited nodes
    let mut todo_nodes: Vec<ClassId> = Vec::new();
    // if completing this node also means the parent is done
    let mut todo_finishes: Vec<bool> = Vec::new();
    // critical path
    // map eclass id -> MD
    // not in map -> not in a critical path
    let mut critical_path: HashMap<ClassId,usize> = HashMap::new();

    let mut ckt_md = 0;
    for o_id in out_net_to_eclass.values() {
        let o_id = ClassId::new(Into::<u32>::into(*o_id));
        todo_nodes.push(o_id);
        todo_finishes.push(false);
        let md = cost_analysis.get(&o_id).unwrap().0;
        if md > ckt_md {
            ckt_md = md;
        }
    }
    // add the bases to the critical path
    for o_id in out_net_to_eclass.values() {
        let o_id = ClassId::new(Into::<u32>::into(*o_id));
        let md = cost_analysis.get(&o_id).unwrap().0;
        if md == ckt_md {
            critical_path.insert(o_id, md);
        }
    }

    while !todo_nodes.is_empty() {
        let eclass = todo_nodes.pop().unwrap();
        let md = critical_path.get(&eclass).cloned();
        let mut netd = format!("n{} = ", eclass);

        // number of children this node introduces
        // may not be fixed if the eclasses have already been visited
        let mut new_children = 0;
        let mut is_and = false;
        let already_seen = eclass_seen.get(&eclass).is_some();
        let already_complete = already_seen && md.is_some();
        if !already_complete {
            let enodeid = &cost_analysis.get(&eclass).unwrap().1;
            let enode = &egraph[enodeid];
            eclass_seen.insert(eclass);
            let mut children: Option<&[NodeId]> = Some(&enode.children);
            match crate::serde::decode_op_string(&enode.op) {
                PropId::And => {
                    let a = enode.children[0].class();
                    let b = enode.children[1].class();
                    netd.push_str(format!("n{} * n{};", a, b).as_str());
                    is_and = true;
                }
                PropId::Xor => {
                    let a = enode.children[0].class();
                    let b = enode.children[1].class();
                    netd.push_str(format!("n{} ^ n{};", a, b).as_str());
                    //netd.push_str(format!("(!n{} * n{}) + (n{} * !n{});", a, b, a, b).as_str());
                }
                PropId::Not => {
                    let a = enode.children[0].class();
                    netd.push_str(format!("!n{};", a).as_str());
                }
                PropId::Sym => {
                    children = None;
                    netd.push_str(&enode.op);
                    netd.push(';');
                }
                PropId::Lit => {
                    children = None;
                    netd.push_str(if &enode.op == "true" {"1;"} else {"0;"} );
                }
            }
            if let Some(children) = children {
                if let Some(md) = md { 
                    for child_node in children {
                        // on the critical path
                        let child_node = ClassId::new(child_node.class());
                        let child_md = cost_analysis.get(&child_node).unwrap().0;
                        let is_critical = (is_and && child_md == md - 1) || (!is_and && child_md == md);
                        if is_critical || (eclass_seen.get(&child_node).is_none() && !already_seen) {
                            todo_nodes.push(child_node);
                            if is_critical { critical_path.insert(child_node, child_md);} 
                            if !already_seen { new_children += 1;}
                        }
                    }
                } else if !already_seen {
                    for child_node in children {
                        let child_node = ClassId::new(child_node.class());
                        if eclass_seen.get(&child_node).is_none() {
                            todo_nodes.push(child_node);
                            new_children += 1;
                        }
                    }
                }
            }
        }
        // "leaf" node
        // either an actual leaf or all of its children were visited already
        // or the node itself was already visited
        if new_children == 0 {
            if !already_seen {
                real_network.push(netd);
                topo_cost_analysis.insert(eclass, *cost_analysis.get(&eclass).unwrap());
            }
            while let Some(is_finish_v) = todo_finishes.pop() {
                if !is_finish_v {
                    break;
                }
                // all of its children are done, so this equation can be added
                real_network.push(network.pop().unwrap().to_string());
                topo_cost_analysis.insert(eclass, *cost_analysis.get(&eclass).unwrap());
            }
        } else {
            // last child only triggers walking up the stack
            for i in 0..new_children {
                if i == 0 {
                    todo_finishes.push(true);
                } else {
                    todo_finishes.push(false);
                }
            }
            network.push(netd);
        }
    }
    assert!(todo_finishes.is_empty());

    //println!("Critical path: {}% of ckt", (critical_path.len() as f64)/(eclass_seen.len() as f64) * 100.);

    for (o_name, o_id) in out_net_to_eclass.iter() {
        real_network.push(format!("{} = n{};", o_name, o_id));
    }

    // yikes
    //*cost_analysis = cost_analysis.clone().into_iter().filter(|(k,_)| eclass_seen.contains(k)).collect();
    //(critical_path, 
    real_network.join("\n")
}
