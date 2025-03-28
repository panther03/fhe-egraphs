// Extraction algorithms working (primarily) on the
// unserialized E-Graph (from egg itself)
use egg::*;
use indexmap::IndexMap;

use std::usize::MAX;
use std::collections::HashMap;

use egraph_serialize::NodeId;
use std::f64::INFINITY;

use crate::common::{Prop,DepthArea};

pub struct MultComplexity;
impl egg::CostFunction<Prop> for MultComplexity {
    type Cost = usize;
    fn cost<C>(&mut self, enode: &Prop, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let op_cost = match enode {
            Prop::And(..) => 1,
            _ => 0,
        };
        enode.fold(op_cost, |sum, i| sum + costs(i))
    }
}

//impl<N: Analysis<Prop>> LpCostFunction<Prop, N> for MultComplexity {
//    fn node_cost(&mut self, _egraph: &EGraph<Prop, N>, _eclass: Id, enode: &Prop) -> f64 {
//        match enode {
//            Prop::And(..) => 1.0,
//            _ => 0.0,
//        }
//    }
//}

pub struct MultDepth;
impl egg::CostFunction<Prop> for MultDepth {
    type Cost = usize;
    fn cost<C>(&mut self, enode: &Prop, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let op_cost = match enode {
            Prop::And(..) => 1,
            _ => 0,
        };
        op_cost + enode.fold(0, |max, i| max.max(costs(i)))
    }
}

pub struct EsynDepth;
impl egg::CostFunction<Prop> for EsynDepth {
    type Cost = DepthArea;
    fn cost<C>(&mut self, enode: &Prop, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let base = enode.fold(DepthArea {depth: 0, area: 0.0}, |sum, i| sum + costs(i));
        match enode {
            Prop::And(..) => DepthArea {
                depth: base.depth + 22,
                area: base.area + 22.,
            },
            Prop::Or(..) => DepthArea {
                depth: base.depth + 26,
                area: base.area + 26.,
            },
            Prop::Not(..) => DepthArea {
                depth: base.depth + 9,
                area: base.area + 9.
            },
            _ => DepthArea {
                depth: 0,
                area: 0.0
            },
        }        
    }
}

#[allow(unused)]
pub fn dag_md_traversal<'a, N>(
    cost_analysis: &MixedCost<'a, Prop, N>,
    outnodes: &str,
    outnode_ids: &Vec<Id>,
) -> (HashMap<Id,usize>,String)
where
    N: Analysis<Prop>,
{
    // temporary network to hold nodes whose children have not been visited yet
    let mut network: Vec<String> = Vec::new();
    // the network which we will output
    let mut real_network: Vec<String> = Vec::new();
    // set of visited nodes
    let mut eclass_seen: HashMap<Id, Id> = HashMap::new();

    // stack of visited nodes
    let mut todo_nodes: Vec<Id> = Vec::new();
    // if completing this node also means the parent is done
    let mut todo_finishes: Vec<bool> = Vec::new();
    // critical path
    // map eclass id -> MD
    // not in map -> not in a critical path
    let mut critical_path: HashMap<Id,usize> = HashMap::new();

    let mut ckt_md = 0;
    for o_id in outnode_ids {
        todo_nodes.push(*o_id);
        todo_finishes.push(false);
        let md = cost_analysis.results.get(o_id).unwrap().0;
        if md > ckt_md {
            ckt_md = md;
        }
    }
    // add the bases to the critical path
    for o_id in outnode_ids {
        let md = cost_analysis.results.get(o_id).unwrap().0;
        if md == ckt_md {
            critical_path.insert(*o_id, md);
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
            let enode = &cost_analysis.results.get(&eclass).unwrap().1;
            eclass_seen.insert(eclass, eclass);
            let mut children: Option<&[Id]> = None;
            match enode {
                Prop::And(and_children) => {
                    let a = and_children[0];
                    let b = and_children[1];
                    netd.push_str(format!("n{} * n{};", a, b).as_str());
                    children = Some(and_children.as_slice());
                    is_and = true;
                }
                Prop::Xor(xor_children) => {
                    let a = xor_children[0];
                    let b = xor_children[1];
                    netd.push_str(format!("(!n{} * n{}) + (n{} * !n{});", a, b, a, b).as_str());
                    children = Some(xor_children.as_slice());
                }
                Prop::Not(a) => {
                    netd.push_str(format!("!n{};", a).as_str());
                    children = Some(a.as_slice());
                }
                Prop::Symbol(s) => {
                    netd.push_str(s.as_str());
                    netd.push(';');
                }
                Prop::Bool(b) => {
                    netd.push_str(if *b { "1;" } else { "0;" });
                }
                _ => {}
            }
            if let Some(children) = children {
                if let Some(md) = md { 
                    for child_node in children {
                        // on the critical path
                        let child_md = cost_analysis.results.get(child_node).unwrap().0;
                        let is_critical = (is_and && child_md == md - 1) || (!is_and && child_md == md);
                        if is_critical || (eclass_seen.get(child_node).is_none() && !already_seen) {
                            todo_nodes.push(*child_node);
                            if is_critical { critical_path.insert(*child_node, child_md);} 
                            if !already_seen { new_children += 1;}
                        }
                    }
                } else if !already_seen {
                    for child_node in children {
                        if eclass_seen.get(child_node).is_none() {
                            todo_nodes.push(*child_node);
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
            }
            while let Some(is_finish_v) = todo_finishes.pop() {
                if !is_finish_v {
                    break;
                }
                // all of its children are done, so this equation can be added
                real_network.push(network.pop().unwrap().to_string());
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

    for (o_id, o_name) in outnode_ids.iter().zip(outnodes.split(" ")) {
        real_network.push(format!("{} = n{};", o_name, o_id));
    }

    (critical_path, real_network.join("\n"))
}

pub fn recexpr_traversal(expr: RecExpr<Prop>, out_net_to_eclass: &IndexMap<String, Id>) -> String {
    let mut network: Vec<String> = Vec::new();

    for (id, p) in expr.as_ref().iter().enumerate() {
        let mut netd = format!("n{id} = ");
        let mut ok = true;
        match p {
            Prop::And([a, b]) => {
                netd.push_str(format!("n{} * n{};", a, b).as_str());
            }
            Prop::Or([a, b]) => {
                netd.push_str(format!("n{} + n{};", a, b).as_str());
            }
            Prop::Xor([a, b]) => {
                netd.push_str(format!("(!n{} * n{}) + (n{} * !n{});", a, b, a, b).as_str());
            }
            Prop::Not(a) => {
                netd.push_str(format!("!n{};", a).as_str());
            }
            Prop::Symbol(s) => {
                netd.push_str(s.as_str());
                netd.push(';');
            }
            Prop::Bool(b) => {
                netd.push_str(if *b { "true;" } else { "false;" });
            }
            Prop::Concat(outs ) => {
                for ((o_name, _), o_id) in out_net_to_eclass.iter().zip(outs.iter()) {
                    network.push(format!("{} = n{};", o_name, o_id));            
                }
                ok = false;
            }
            _ => { ok = false; }
        }
        if ok {network.push(netd);}
    }

    network.join("\n")
}

#[allow(unused)]
pub struct MixedCost<'a, L: Language, N: Analysis<L>> {
    pub egraph: &'a EGraph<L,N>,
    pub enode_opt_lookup: HashMap<egraph_serialize::NodeId, f64>,
    pub results: HashMap<Id, (usize, Prop)>,
    pub visited: HashMap<Id,Id>
}
impl <'a, N: Analysis<Prop>> MixedCost<'a, Prop, N> {
    #[allow(unused)]
    pub fn select_best_eclass_mixed(&mut self, eclass: Id, depth: usize) -> usize {
        let mut best_prop: Option<Prop> = None;
        let mut best_cost: DepthArea = DepthArea::max();
        let eclass_s = &self.egraph[eclass];
        if self.visited.get(&eclass).is_some() {
            // cycle detected, ignore this node
            return MAX;
        }
        self.visited.insert(eclass, eclass);
        for (i, node) in eclass_s.nodes.iter().enumerate() {
            let worst_depth:usize = node.children().iter().map(|c| {
                // filter out cycles, although these shouldn't ei
                self.results.get(c).map(|x| { x.0}).unwrap_or_else(|| self.select_best_eclass_mixed
                    
                    (*c, depth+1))
            }).max().unwrap_or(0);
            if worst_depth == MAX {
                continue;
            }
            let node_id_ser: NodeId = format!("{}.{}", eclass_s.id, i).into();
            let dag_area = *self.enode_opt_lookup.get(&node_id_ser).unwrap_or(&INFINITY);
            let md_cost = match node {
                Prop::And(_) => 1,
                _ => 0
            };
            let node_cost = DepthArea {
                area: dag_area,
                depth: worst_depth + md_cost
            };
            // this makes a big difference in the quality of area results for some reason???
            if node_cost < best_cost {
                best_cost = node_cost;
                best_prop = Some(node.clone());
            }
        }
        self.results.insert(eclass, (best_cost.depth, best_prop.unwrap()));
        return best_cost.depth;
    }
}