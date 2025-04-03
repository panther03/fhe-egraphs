use std::{
    collections::{HashMap, HashSet}, hash::Hash, io::Empty, sync::{Arc, Mutex}, usize::MAX
};

use egraph_serialize::{ClassId, EGraph, NodeId};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};

use crate::{extraction_ser::ser_egraph_to_dot, traverse::{self, should_visit_complete_class}};
use rayon::prelude::*;

trait TraverseData<T: Clone = Self>: Clone {
    fn root_data() -> Self;
}

pub enum TraversalWorkItem<T> {
    Child(T, ClassId),
    Continuation(T, NodeId),
}

type EGraphWorkList<T> = Vec<TraversalWorkItem<T>>;

trait EGraphTraversalResponder<T: TraverseData> {
    fn handle_root(&mut self, root: ClassId);

    fn handle_child(
        &mut self,
        egraph: &EGraph,
        worklist: &mut EGraphWorkList<T>,
        data: T,
        class: ClassId,
    );

    fn handle_cont(
        &mut self,
        egraph: &EGraph,
        worklist: &mut EGraphWorkList<T>,
        data: T,
        node: NodeId,
    );
}

pub fn egraph_traverse<T: TraverseData, R: EGraphTraversalResponder<T>>(
    egraph: &EGraph,
    _roots: &[ClassId],
    responder: &mut R,
) {
    let mut worklist: EGraphWorkList<T> = EGraphWorkList::new();

    for root in _roots {
        worklist.push(TraversalWorkItem::Child(T::root_data(), *root));
        responder.handle_root(*root);
    }

    while !worklist.is_empty() {
        let item = worklist.pop().unwrap();
        match item {
            TraversalWorkItem::Child(data, class) => {
                //worklist.push(TraversalWorkItem::Continuation(class));
                responder.handle_child(egraph, &mut worklist, data, class);
            }
            TraversalWorkItem::Continuation(data, node) => {
                responder.handle_cont(egraph, &mut worklist, data, node);
            }
        }
    }
}

type FlatNode = usize;
#[derive(Clone)]
struct ParentEnode(Option<FlatNode>);
impl TraverseData for ParentEnode {
    fn root_data() -> Self {
        ParentEnode(None)
    }
}

#[derive(Clone)]
pub struct EmptyContext();
impl TraverseData for EmptyContext {
    fn root_data() -> Self {
        EmptyContext()
    }
}

#[derive(Clone)]
struct EclassInd(usize);
impl TraverseData for EclassInd {
    fn root_data() -> Self {
        EclassInd(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlackCost {
    Visited(usize),
    Infinite,
    Unvisited,
    Filtered,
}

pub struct SlackNaive {
    pub md_lookup: HashMap<ClassId, SlackCost>,
    pub base_class: Option<ClassId>,
    pub visited: HashSet<ClassId>
}

impl SlackNaive {
    pub fn new(base_class: ClassId) -> Self {
        Self {
            md_lookup: HashMap::new(),
            base_class: Some(base_class),
            visited: HashSet::new()
        }
    }

    pub fn new_all_ckt() -> Self {
        Self {
            md_lookup: HashMap::new(),
            base_class: None,
            visited: HashSet::new()
        }
    }

    /*
    fn compute_md(&mut self, egraph: &EGraph, class: egraph_serialize::ClassId) {
        self.visited.insert(class);
        // unvisited
        let mut worst_md: Option<usize> = None;
        for node in &egraph[&class].nodes {
            // FilteredUnknown
            let mut node_md: Option<usize> = if self.base_class.is_some() {
                None
            } else {
                Some(0)
            };
            for child in &egraph[node].children {
                let child = ClassId::new(child.class());
                let child_md = self.md_lookup.get(&child).map(|v| *v).unwrap();
                // If this child is filtered
                // Status of node stays the same (FilteredUnknown > Filtered)
                // Else it is Visited(v) Status of node is updated if the node is filteredunknown (Visited(v) > FilteredUnknown))
                // Otherwise it is Visited(v1) > Visited(v2)
                node_md = child_md.map_or(node_md, |cmd| {
                    node_md.map_or(Some(cmd), |md| Some(md.max(cmd)))
                });
            }
            // only care about this node if it reaches the base
            // if worst_md is filteredunknown
            // Then it is auto updated as filteredunknown (less than or equal to)
            // else if it is visited(v) status of node is updated if it is unvisited (Visited(v) < Unvisited)
            if let Some(node_md) = node_md {
                let total_md = node_md + (&egraph[node]).cost.round() as usize;
                worst_md = match worst_md {
                    None => Some(total_md),
                    Some(worst_md) => Some(worst_md.min(total_md)),
                };
            }
        }
        self.md_lookup.insert(class, worst_md);
    }*/
}

// too much for my brain
/*

impl Ord for SlackCost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self {
        Self::Filtered => if Visited()
        }
    }
}

impl traverse::CostVal for SlackCost {
    fn unvisited() -> Self {
        Self::Unvisited
    }
    fn from_egraph_cost(cost: egraph_serialize::Cost) -> Self {
        Self::Visited(cost.round() as usize)
    }
}*/

impl SlackCost {
    fn max_by_node_branch(&self, other: &Self) -> Self {
        match self {
            Self::Unvisited => *self,
            Self::Infinite => if let Self::Unvisited = other {Self::Unvisited} else {Self::Infinite},
            Self::Visited(v1) => match other {
                Self::Filtered => *self,
                Self::Visited(v2) if v1 > v2 => *self,
                _ => *other
            },
            Self::Filtered => *other
        }
    }

    fn min_by_node_in_class(&self, other: &Self) -> Self {
        match self {
            Self::Unvisited => *self,
            Self::Infinite => if let Self::Unvisited = other {Self::Unvisited} else {*other},
            Self::Visited(v1) => match other {
                Self::Filtered => *self,
                Self::Visited(v2) if v1 < v2 => *self,
                _ => *other
            },
            Self::Filtered => match other {
                Self::Infinite => *self,
                _ => *other
            }
        }
    }

    fn add_cost(self, cost: usize) -> Self {
        match self {
            Self::Visited(v1) => Self::Visited(v1 + cost),
            _ => self
        }
    }

    fn unwrap_visited(&self) -> usize {
        match self {
            Self::Visited(v) => *v,
            _ => panic!("failed to unwrap slackcost")
        }
    }
}

impl traverse::EGraphVisitor for SlackNaive {
    fn visit(&mut self, egraph: &EGraph, class: &egraph_serialize::Class) -> bool {
        if Some(class.id) == self.base_class {
            if !self.md_lookup.contains_key(&class.id) {
                self.md_lookup.insert(class.id,SlackCost::Visited(0));
                return true;
            } else {
                return false;
            }
        }
        let worst_md = *self.md_lookup.get(&class.id).unwrap_or(&SlackCost::Unvisited);
        // maybe it shouldnt be like this
        let mut new_worst_md = worst_md;
        let mut non_unvisited_encountered = false;
        for node in &egraph[&class.id].nodes {
            let mut node_md: SlackCost = if self.base_class.is_some() {
                SlackCost::Filtered
            } else {
                SlackCost::Visited(0)
            };
            
            for child in &egraph[node].children {
                let child = ClassId::new(child.class());
                let child_md = self.md_lookup.get(&child).map(|v| *v).unwrap_or(SlackCost::Unvisited);
                node_md = node_md.max_by_node_branch(&child_md);
            }

            non_unvisited_encountered = non_unvisited_encountered || (node_md != SlackCost::Unvisited); 
            node_md = node_md.add_cost((&egraph[node]).cost.round() as usize);
            new_worst_md = new_worst_md.min_by_node_in_class(&node_md);
        }
        if new_worst_md == SlackCost::Unvisited && non_unvisited_encountered {
            new_worst_md = SlackCost::Infinite;    
        }
        if new_worst_md != worst_md {
            self.md_lookup.insert(class.id, new_worst_md);
            return true;
        } else {
            return false;
        }
    }
}

/*
impl EGraphTraversalResponder<EmptyContext> for SlackNaive {
    fn handle_child(
        &mut self,
        egraph: &EGraph,
        worklist: &mut EGraphWorkList<EmptyContext>,
        _: EmptyContext,
        class: ClassId,
    ) {
        if Some(class) == self.base_class {
            self.md_lookup.insert(class, Some(0));
        } else {
            worklist.push(TraversalWorkItem::Continuation(
                EmptyContext(),
                NodeId::new(0, class.class()),
            ));
            for node in &egraph[&class].nodes {
                for child in &egraph[node].children {
                    let child = egraph.nid_to_cid(child);
                    // 
                    // self.visited.contains(child)
                    if self.md_lookup.get(child).is_none() {
                        //self.md_lookup.insert(*child, None);
                        worklist.push(TraversalWorkItem::Child(EmptyContext(), *child));
                    }
                }
            }
        }
    }

    fn handle_cont(
        &mut self,
        egraph: &EGraph,
        _: &mut EGraphWorkList<EmptyContext>,
        _: EmptyContext,
        node: NodeId,
    ) {
        self.compute_md(egraph, ClassId::new(node.class()));
    }

    fn handle_root(&mut self, _: ClassId) {}
}
*/
#[derive(Debug)]
pub struct NodeBounds {
    pub inv_md: usize,
    pub parent_node: Option<FlatNode>,
    pub class: ClassId,
    pub refcnt: usize,
}

pub struct MdBounds {
    pub flat_node_lookup: HashMap<NodeId, usize>,
    pub node_data: Vec<NodeBounds>,
    pub worst_md: usize,
}

pub struct FanoutData<'a> {
    flat_node_lookup: &'a mut HashMap<NodeId, usize>,
    node_data: &'a mut Vec<NodeBounds>,
    path: Vec<NodeId>,
    path_set: HashSet<NodeId>,
}

impl<'a> FanoutData<'a> {
    fn from_bounds(bounds: &'a mut MdBounds) -> Self {
        Self {
            flat_node_lookup: &mut bounds.flat_node_lookup,
            node_data: &mut bounds.node_data,
            path: Vec::new(),
            path_set: HashSet::new(),
        }
    }
}

impl<'a> EGraphTraversalResponder<EclassInd> for FanoutData<'a> {
    fn handle_child(
        &mut self,
        egraph: &EGraph,
        worklist: &mut EGraphWorkList<EclassInd>,
        _: EclassInd,
        class: ClassId,
    ) {
        let num_enodes = egraph[&class].nodes.len();
        for (i, node) in egraph[&class].nodes.iter().enumerate() {
            let flat_node = self.flat_node_lookup.get(node);

            if let Some(flat_node) = flat_node {
                // cycle detected
                if self.path_set.contains(node) {
                    let ind = self.path.len() - 1;
                    while self.path[ind] != *node {
                        let path_node = self.path[ind];
                        let path_class = ClassId::new(path_node.class());
                        let flat_path_node = self.flat_node_lookup.get(&path_node).unwrap();
                        //println!("refcnt decreased {}", flat_path_node);
                        self.node_data[*flat_path_node].refcnt -= 1;
                        if !egraph[&path_class].nodes.iter().all(|n| {
                            let fpn = self.flat_node_lookup.get(n).unwrap();
                            self.node_data[*fpn].refcnt == 0
                        }) {
                            break;
                        }
                    }
                } else {
                    self.node_data[*flat_node].refcnt += 1;
                }
            } else {
                let cost = (&egraph[node]).cost.round() as usize;
                let fresh = self.node_data.len();
                self.node_data.push(NodeBounds {
                    inv_md: cost,
                    parent_node: None,
                    class: class,
                    refcnt: 1,
                });
                self.flat_node_lookup.insert(*node, fresh);

                if i != 0 {
                    worklist.push(TraversalWorkItem::Continuation(EclassInd(i), *node));
                }
                if i == num_enodes - 1 {
                    self.path.push(*node);
                    self.path_set.insert(*node);
                }
                worklist.push(TraversalWorkItem::Continuation(EclassInd(0), *node));
                for child in &egraph[node].children {
                    let child = egraph.nid_to_cid(child);
                    worklist.push(TraversalWorkItem::Child(EclassInd(0), *child));
                }
            }
        }
    }
    fn handle_cont(
        &mut self,
        egraph: &EGraph,
        _: &mut EGraphWorkList<EclassInd>,
        data: EclassInd,
        node: NodeId,
    ) {
        match data {
            EclassInd(0) => {
                self.path.pop();
                self.path_set.remove(&node);
            }
            EclassInd(i) => {
                let class = ClassId::new(node.class());
                let node = egraph[&class].nodes[i - 1];
                self.path.push(node);
                self.path_set.insert(node);
            }
        }
    }
    fn handle_root(&mut self, _: ClassId) {}
}

// TODO: flatten...? what did i mean by this?
impl MdBounds {
    fn new() -> Self {
        Self {
            flat_node_lookup: HashMap::new(),
            node_data: Vec::new(),
            worst_md: 0,
        }
    }

    fn compute_fanouts(&mut self, egraph: &EGraph, _roots: &[ClassId]) {
        egraph_traverse(egraph, _roots, &mut FanoutData::from_bounds(self));
    }

    pub fn extract(egraph: &EGraph, _roots: &[ClassId]) -> Self {
        let mut this = Self::new();
        this.compute_fanouts(egraph, _roots);
        egraph_traverse(egraph, _roots, &mut this);
        this
    }

    fn compat(&self, n1: FlatNode, n2: FlatNode) -> bool {
        let mut visited: HashSet<FlatNode> = HashSet::new();
        let mut classes: HashSet<ClassId> = HashSet::new();
        let mut n = n1;
        loop {
            let node_data = &self.node_data[n];
            visited.insert(n);
            classes.insert(node_data.class);
            if let Some(p) = node_data.parent_node {
                n = p;
            } else {
                break;
            }
        }
        n = n2;
        loop {
            let node_data = &self.node_data[n];
            if visited.contains(&n) {
                return true;
            } else if classes.contains(&node_data.class) {
                return false;
            } else if let Some(p) = node_data.parent_node {
                n = p;
            } else {
                return true;
            }
        }
    }
}

impl EGraphTraversalResponder<ParentEnode> for MdBounds {
    fn handle_child(
        &mut self,
        egraph: &EGraph,
        worklist: &mut EGraphWorkList<ParentEnode>,
        data: ParentEnode,
        class: ClassId,
    ) {
        for node in &egraph[&class].nodes {
            let flat_node = *self.flat_node_lookup.get(node).unwrap();
            let cost = (&egraph[node]).cost.round() as usize;
            let refcnt = self.node_data[flat_node].refcnt;

            if refcnt == 0 {
                continue;
            }

            match data {
                ParentEnode(None) => {}
                ParentEnode(Some(pn)) => {
                    let pn_inv_md = self.node_data[pn].inv_md;
                    match self.node_data[flat_node].parent_node {
                        None => {
                            let node_data = &mut self.node_data[flat_node];
                            node_data.parent_node = Some(pn);
                            node_data.inv_md = cost + pn_inv_md;
                        }
                        Some(p) => {
                            let p_inv_md = self.node_data[p].inv_md;
                            let c = self.compat(p, pn);
                            //if c {
                            //    println!("{}({},{}) and {}({},{}) are compatible!", p, self.node_data[p].class, p_inv_md,  pn, self.node_data[pn].class, pn_inv_md);
                            //} else {
                            //    println!("{}({},{}) and {}({},{}) are incompatible!", p, self.node_data[p].class, p_inv_md,  pn, self.node_data[pn].class, pn_inv_md);
                            //}
                            if (c && pn_inv_md > p_inv_md) || (!c && pn_inv_md < p_inv_md) {
                                //println!("choosing {} over {}", pn, p);
                                let node_data = &mut self.node_data[flat_node];
                                node_data.parent_node = Some(pn);
                                node_data.inv_md = cost + pn_inv_md;
                            }
                        }
                    }
                    let inv_md = self.node_data[flat_node].inv_md;
                    if inv_md > self.worst_md {
                        self.worst_md = inv_md;
                        dbg!(self.worst_md);
                    }
                }
            }
            assert!(refcnt > 0);
            self.node_data[flat_node].refcnt -= 1;
            if self.node_data[flat_node].refcnt == 0 {
                // traverse children
                for child in &egraph[node].children {
                    let child = egraph.nid_to_cid(child);
                    worklist.push(TraversalWorkItem::Child(
                        ParentEnode(Some(flat_node)),
                        *child,
                    ));
                }
            }
        }
    }
    fn handle_cont(
        &mut self,
        _: &EGraph,
        _: &mut EGraphWorkList<ParentEnode>,
        _: ParentEnode,
        _: NodeId,
    ) {
    }
    fn handle_root(&mut self, _: ClassId) {}
}

pub fn calc_bounds(
    egraph: &EGraph,
    _roots: &[ClassId],
) -> (SlackNaive, usize, HashMap<ClassId, i32>) {
    // should not be in the e-graph!
    let mut all_md = SlackNaive::new_all_ckt();
    traverse::egraph_pass_traverse(&egraph, &mut all_md);
    dbg!(all_md.visited.len());
    dbg!(egraph.classes().len());
    let ckt_md = _roots
        .iter()
        .map(|r| all_md.md_lookup.get(r).unwrap_or(&SlackCost::Unvisited).unwrap_visited() )
        .max()
        .unwrap();
    dbg!(ckt_md);

    let mut bounds = HashMap::new();
    let pb = ProgressBar::new(egraph.classes().len() as u64);
    /*pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos:>7}/{len:7} {msg}")
    .unwrap()
    .progress_chars("#>-"));

    let mut cnt = 0;
    // could consider parallelizing this
    for (cid, _) in egraph.classes() {
        let mut slack = SlackNaive::new(*cid);
        let iters = traverse::egraph_pass_traverse(&egraph, &mut slack);
        cnt += 1;
        pb.set_position(cnt);
        pb.set_message(format!("iters={}", iters));
        let mut worst_remaining: Option<usize> = None;
        for root in _roots {
            worst_remaining = slack.md_lookup.get(root).map_or(worst_remaining, |rmd| {
                match rmd {
                    SlackCost::Visited(rmd2) => Some(worst_remaining.map_or(*rmd2, |wrm: usize| wrm.max(*rmd2))),
                    _ => worst_remaining    
                }
            });
        }
        if let Some(worst_remaining) = worst_remaining {
            ckt_remaining.insert(*cid, worst_remaining);
        }
        drop(slack);
    }*/
    // Create the progress bar
    let pb = ProgressBar::new(egraph.classes().len() as u64);
    //pb.set_style(ProgressStyle::default_bar()
    //    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
    //    .unwrap()
    //    .progress_chars("#>-"));
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos:>7}/{len:7} {msg}")
    .unwrap()
    .progress_chars("#>-"));


    // Wrap the progress bar in an Arc<Mutex> for thread-safe sharing
    let pb = Arc::new(Mutex::new(pb));

    let classes: Vec<(&ClassId, &egraph_serialize::Class)> = egraph.classes().iter().collect();
    let results: Vec<_> = classes
        .par_iter()
        .map(|(cid, _)| {
            let mut slack = SlackNaive::new(**cid);
            let iters: usize = traverse::egraph_pass_traverse(&egraph, &mut slack);
            
            // Update progress bar safely
            {
                let pb = pb.lock().unwrap();
                pb.inc(1);
                pb.set_message(format!("iters={}", iters));
            }
            
            let mut worst_remaining: Option<usize> = None;
            for root in _roots {
                worst_remaining = slack.md_lookup.get(root).map_or(worst_remaining, |rmd| {
                    match rmd {
                        SlackCost::Visited(rmd2) => Some(worst_remaining.map_or(*rmd2, |wrm: usize| wrm.max(*rmd2))),
                        _ => worst_remaining    
                    }
                });
            }

            if worst_remaining.is_none() {
                println!("eclass w/ no path to root: {}", cid);
            }
            
            let result = worst_remaining.map(|wr| (**cid, wr));
            drop(slack);
            result
        })
        .filter_map(|x| x)
        .collect();

    // Finish the progress bar
    pb.lock().unwrap().finish();

    // Insert results into the HashMap
    for (cid, worst_remaining) in results {
        bounds.insert(cid, (ckt_md as i32) - (worst_remaining as i32));
    }
    dbg!("b");
    (all_md, ckt_md, bounds)
}

#[derive(Clone, Copy, PartialEq)]
enum PruneMark {
    //PrunedPendingProp,
    //PruneMarked,
    //PruneMarkedTwice,
    Pruned
}

struct PruneFirstMarkTraverse<'a> {
    pruned: HashMap<ClassId, PruneMark>,
    slack: &'a SlackNaive,
    bounds: &'a HashMap<ClassId, i32>,
//    reachable_memo: HashMap<(ClassId, ClassId), bool>
}


impl<'a> traverse::EGraphVisitor for PruneFirstMarkTraverse<'a> {
    /*fn iter_start(&mut self) {
        self.pruned.iter_mut().for_each(|(_, mark)| {
            *mark = match mark {
            PruneMark::PruneMarkedTwice => PruneMark::PrunedPendingProp,
            _ => *mark
            };
        })
    }*/

    fn visit(&mut self, egraph: &EGraph, class: &egraph_serialize::Class) -> bool {
        let class = class.id;
        match self.pruned.get(&class).cloned() {
            None => {
                if !self.bounds.contains_key(&class) {
                    self.pruned.insert(class,PruneMark::Pruned);
                    return true;
                }

                let mut did_something = false;
                let bound = self.bounds.get(&class).unwrap();
                let mut definitely_pruned = true;
                for node in &egraph[&class].nodes {
                    let mut md_child = 0;
                        let mut are_children_pruned = false;
                        for child in &egraph[node].children {
                            let child = ClassId::new(child.class());
                            md_child = md_child.max(self.slack.md_lookup.get(&child).unwrap().unwrap_visited());
                            match self.pruned.get(&child).cloned() {
                                //Some(PruneMark::PruneMarked) | Some(PruneMark::PruneMarkedTwice) if !self.reachable(egraph, child, class) => {
                                //    did_something = true;
                                //    self.pruned.remove(&child);
                                //}
                                Some(PruneMark::Pruned) => {
                                    are_children_pruned = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        let cost = (&egraph[node]).cost.round() as usize;
                        if !are_children_pruned && (cost + md_child) as i32 <= *bound {
                            // as far as we know this node is not getting touched
                            // so the class shouldn't either.
                            definitely_pruned = false;
                        }
                }

                if definitely_pruned {
                    // if we made it here all the nodes were pruned. so this class is pruned too and it wasn't before.
                    self.pruned.insert(class, PruneMark::Pruned);
                    did_something = true;
                }
                did_something
            }
            //Some(PruneMark::PruneMarked) | Some(PruneMark::PruneMarkedTwice) => {
            //    self.pruned.insert(class,PruneMark::PruneMarkedTwice);
            //    return true;
            //}
            //Some(PruneMark::PrunedPendingProp) => {
            //    // we automatically put a mark on all these nodes
            //    // there is 1 full iteration for the mark to be cleared
            //    for node in &egraph[&class].nodes {
            //        for child in &egraph[node].children {
            //            let child = ClassId::new(child.class());
            //            println!("marking {} for kill", child);
            //            if !self.pruned.contains_key(&child) {
            //                self.pruned.insert(child, PruneMark::PruneMarked);
            //            }
            //        }
            //    }
            //    self.pruned.insert(class, PruneMark::Pruned);
            //    true
            //}
            _ => false
        }       
    }
}

struct PruneReconstructTraverse<'a> {
    unser_egraph: egg::EGraph<crate::Prop, ()>,
    ser_to_unser: HashMap<egg::Id, egg::Id>,
    node_visited: HashSet<NodeId>,
    pruned: &'a HashMap<ClassId, PruneMark>,
}

impl <'a> traverse::EGraphVisitor for PruneReconstructTraverse<'a> {
    fn visit(&mut self, egraph: &EGraph, class: &egraph_serialize::Class) -> bool {
        let mut did_something = false;
        let cid = class.id;
        let ser_id = egg::Id::from(cid.class() as usize);
        let mut unser_id = self.ser_to_unser.get(&ser_id).cloned();
        for node in &egraph[&cid].nodes {
            if !self.node_visited.contains(node) && 
            !self.pruned.contains_key(&ClassId::new(node.class())) {
                let enode: Option<crate::common::Prop> = crate::serde::decode_enode(
                    &self.ser_to_unser,
                    &egraph[node],
                );
                if let Some(enode) = enode {
                    self.node_visited.insert(*node);
                    did_something = true;
                    let id = self.unser_egraph.add(enode);
                    /*let id32: u32 = id.into();
                    if id32 == (292 as u32) ||
                        id32 == (298 as u32) ||
                        id32 == (264 as u32) ||
                        id32 == (223 as u32) ||
                        id32 == (280 as u32) ||
                        id32 == (230 as u32) {
                        dbg!(cid);
                    }*/
                    if let Some(unser_id) = unser_id {
                        self.unser_egraph.union(unser_id, id);
                    } else {
                        unser_id = Some(id);
                        self.ser_to_unser.insert(ser_id, id);
                    }
                }
                // OK if it could not be added, this simply means that one of the child nodes
                // was itself pruned, so we don't get to depend on it.
            }
        }
        did_something
    }
}

pub fn egraph_prune(
    egraph: &EGraph,
    _roots: &[ClassId],
    slack: &SlackNaive,
    bounds: &HashMap<ClassId, i32>,
) -> (HashMap::<egg::Id,egg::Id>, egg::EGraph<crate::Prop, ()>) {
    let mut pmt = PruneFirstMarkTraverse {
        pruned: HashMap::new(),
        slack,
        bounds
    };
    traverse::egraph_pass_traverse(egraph, &mut pmt);
    //for (class, mark) in pmt.pruned.iter() {
    //    match mark {
    //        PruneMark::PruneMarked | PruneMark::PruneMarkedTwice => {
    //            panic!("Class {} was left marked upon stop!", class);
    //        }
    //        PruneMark::PrunedPendingProp => {
    //            panic!("Class {} was left pending propagation upon stop!", class);
    //        }
    //        _ => {}
    //    }
    //}

    /*
    optimized doesn't work for some reason because there are classes in the egraph greater than the len()??
    let mut unreachable: Vec<bool> = vec![true; egraph.classes().len()];
    let mut roots_a: HashSet<ClassId> = _roots.iter().map(|c| *c).collect();
    let mut roots_b: HashSet<ClassId> = HashSet::new();

    while !roots_a.is_empty() {
        roots_b.clear();
        for root in &roots_a {
            if !unreachable[root.class() as usize] || pmt.pruned.contains_key(root) {
                continue;
            }
            unreachable[root.class() as usize] = false;
            for node in &egraph[root].nodes {
                for child in &egraph[node].children {
                    let child = ClassId::new(child.class());
                    roots_b.insert(child);
                }
            }
        }
        std::mem::swap(&mut roots_a, &mut roots_b);
    }

    for i in 0..unreachable.len() {
        if unreachable[i] {
            pmt.pruned.insert(ClassId::new(i as u32), PruneMark::Pruned);
        }
    }*/

    let mut unreachable: HashSet<&ClassId> = egraph.classes().iter().map(|(cid,_)| cid).collect();
    let mut roots_a: HashSet<ClassId> = _roots.iter().map(|c| *c).collect();
    let mut roots_b: HashSet<ClassId> = HashSet::new();

    while !roots_a.is_empty() {
        roots_b.clear();
        for root in &roots_a {
            if !unreachable.contains(root) || pmt.pruned.contains_key(root) {
                continue;
            }
            unreachable.remove(root);
            for node in &egraph[root].nodes {
                for child in &egraph[node].children {
                    let child = ClassId::new(child.class());
                    roots_b.insert(child);
                }
            }
        }
        std::mem::swap(&mut roots_a, &mut roots_b);
    }

    for unreachable_class in unreachable {
        pmt.pruned.insert(*unreachable_class, PruneMark::Pruned);
    }

    let mut prt = PruneReconstructTraverse {
        pruned: &pmt.pruned,
        node_visited: HashSet::new(),
        unser_egraph: egg::EGraph::default(),
        ser_to_unser: HashMap::new()
    };
    traverse::egraph_pass_traverse(egraph, &mut prt);

    (prt.ser_to_unser, prt.unser_egraph)
}

/*
pub fn egraph_prune_set( egraph: &EGraph,
    _roots: &[ClassId],
    slack: &SlackNaive,
    ckt_md: usize,
    ckt_remaining: &HashMap<ClassId, usize>) -> HashSet::<NodeId> {
    let mut pruned = HashSet::new();
    for (cid, class) in egraph.classes() {
        if let Some(worst_remaining) = ckt_remaining.get(cid) {
            for node in &class.nodes {
                
                let cost = (&egraph[node]).cost.round() as usize;
                let mut md_child = 0;
                for child in &egraph[node].children {
                    let child = egraph.nid_to_cid(child);
                    md_child = md_child.max(slack.md_lookup.get(child).unwrap().unwrap());
                }
                if cost + md_child + worst_remaining > ckt_md {
                    //println!("pruned {}", node);
                    pruned.insert(*node);
                }
            }
        }
    }
    pruned
}*/

#[cfg(test)]
mod tests {
    use egraph_serialize::{ClassId, NodeId};

    use crate::{
        extraction_ser::{ser_egraph_from_file, ser_egraph_to_dot},
        md_slack::{egraph_traverse, MdBounds, SlackNaive},
        *,
    };

    use super::{calc_bounds, egraph_prune, SlackCost};

    #[test]
    fn spider() {
        let (egraph, out_eclasses) =
            ser_egraph_from_file("/home/julien/EPFL/LSI/work/fhe-egraphs/test.egg");
        //ser_egraph_to_dot(&egraph, "egraph.dot");
        let bounds = MdBounds::extract(&egraph, out_eclasses.as_slice());
        dbg!(bounds.worst_md);
    }

    #[test]
    fn hd08() {
        let egg_file =
            std::fs::File::open("/home/julien/EPFL/LSI/work/fhe-egraphs/hd08.egg").unwrap();
        let egraph = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();
        let out_eclasses = vec![ClassId::new(73)];
        let bounds = MdBounds::extract(&egraph, out_eclasses.as_slice());
        let annot: HashMap<NodeId, usize> = bounds
            .flat_node_lookup
            .iter()
            .map(|(node, flatnode)| (*node, bounds.node_data[*flatnode].inv_md))
            .collect();
        ser_egraph_to_dot(&egraph, &annot, &HashMap::new(), "egraph.dot");
        dbg!(bounds.worst_md);
    }

    #[test]
    fn hd08_dot() {
        let egg_file =
            std::fs::File::open("/home/julien/EPFL/LSI/work/fhe-egraphs/hd08.egg").unwrap();
        let egraph = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();
        ser_egraph_to_dot::<&str>(&egraph, &HashMap::new(), &HashMap::new(), "egraph.dot");
    }

    #[test]
    fn hd08_naive() {
        let egg_file =
            std::fs::File::open("/home/julien/EPFL/LSI/work/fhe-egraphs/hd08_cycles.egg").unwrap();
        let egraph = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();
        //let out_eclasses = vec![ClassId::new(73)];
        let out_eclasses = vec![ClassId::new(7963)];
        
        for i in 0..8 {
            let mut naive = SlackNaive::new(ClassId::new(i));
            traverse::egraph_pass_traverse(&egraph, &mut naive);
            //dbg!(naive.md_lookup);
            //dbg!(naive.worst_md);
    
            //let mut naive = SlackNaive::new_all_ckt();
            //traverse::egraph_pass_traverse(&egraph, &mut naive);
    
            dbg!(naive.md_lookup.get(&out_eclasses[0]).unwrap());
        }
        
        /*

        let annot: HashMap<NodeId, String> = naive
                .md_lookup
                .iter()
                .map(|(class, md)| {
                    (
                        NodeId::new(0, class.class()),
                        match md {
                            SlackCost::Visited(md) => format!("{}", *md as i32),
                            SlackCost::Infinite => String::from("∞"),
                            SlackCost::Filtered => String::from("F"),
                            SlackCost::Unvisited => String::from("U"),
                        },
                    )
                })
                .collect();
         */
        //ser_egraph_to_dot(&egraph, &annot, "egraph.dot");
    }

    #[test]
    fn hd08_prune() {
        let egg_file =
            std::fs::File::open("/home/julien/EPFL/LSI/work/fhe-egraphs/out0.egg").unwrap();
        let egraph = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();
        let out_eclasses = vec![ClassId::new(73)];
        //let out_eclasses = vec![ClassId::new(7963)];
        //let out_eclasses = vec![ClassId::new(390)];

        let (slack, ckt_md, bounds) = calc_bounds(&egraph, &out_eclasses);
        let (ser_to_unser,pruned) = egraph_prune(&egraph, &out_eclasses, &slack, &bounds);
        pruned.dot().to_dot("egraph_pruned.dot").unwrap();
        //let annot: HashMap<NodeId, &str> = egraph.classes().iter()
        //    .map(|(cid, class)| {
        //        (
        //            NodeId::new(0, cid.class()),
        //            if ser_to_unser.contains_key(&egg::Id::from(cid.class() as usize)) {"NP"} else {"P"}
        //        )
        //    })
        //    .collect();

        let annot: HashMap<NodeId, i32> = egraph.nodes.iter().map(|(nid, node)| {
            (*nid, bounds.get(&ClassId::new(nid.class())).map(|b| {
                (ckt_md as i32) - *b + (node.cost.round() as i32)
            }).unwrap_or(-1))
        }).collect();
        dbg!(pruned.total_number_of_nodes());
        dbg!(pruned.number_of_classes());
        dbg!(egraph.classes().len());
        dbg!(egraph.nodes.len());
        let results = global_greedy_dag::mc_extract(&egraph, &out_eclasses, HashMap::new(), &bounds);
        let highlight: HashMap<NodeId, usize> = results.iter().map(|(k,v)| (v.1, v.0)).collect();

        ser_egraph_to_dot::<i32>(&egraph, &annot, &highlight, "egraph.dot");

        /*let pruned = egraph_prune(&egraph, &out_eclasses);
        println!("Pruned {}% of e-graph", (pruned.len() as f32 / egraph.nodes.len() as f32 * 100.));
        
        ser_egraph_to_dot(&egraph, &annot, "egraph.dot");*/
    }
}
