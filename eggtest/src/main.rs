use egg::{rewrite as rw, *};
use egraph_serialize::NodeId;
use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::f64::INFINITY;
use std::fmt::Display;
use std::usize::MAX;
use std::{collections::HashMap, str::FromStr};
use std::time::{Duration, Instant};

mod global_greedy_dag;

define_language! {
    enum Prop {
        Bool(bool),
        "*" = And([Id; 2]),
        "!" = Not(Id),
        "^" = Xor([Id; 2]),
        // used for having multiple outputs
        "$" = Concat(Vec<Id>),
        Symbol(Symbol),
    }
}

fn process_rules(rules_string: &str) -> Vec<Rewrite<Prop, ()>> {
    let mut rules: Vec<Rewrite<Prop, ()>> = vec![
        // Basic commutativity rules, which Lobster assumes
        rw!("0"; "(^ ?x ?y)" => "(^ ?y ?x)"),
        rw!("1"; "(* ?x ?y)" => "(* ?y ?x)"),
        /*rw!("9"; "(! (* (! (* ?x (! ?y))) (! (* (! ?x) ?y))))" => "(^ ?y ?x)"),
        rw!("2"; "(* ?x (^ ?y ?z))" => "(^ (* ?x ?y) (* ?x ?z))"),
        rw!("3"; "(^ (* ?x ?y) (* ?x ?z))" => "(* ?x (^ ?y ?z))"),
        rw!("4"; "(* ?x (* ?y ?z))" => "(* (* ?x ?y) ?z)"),
        rw!("5"; "(^ ?x (^ ?y ?z))" => "(^ (^ ?x ?y) ?z)"),
        rw!("6"; "(^ ?x (* ?x ?y))" => "(* ?x (! ?y))"),
        rw!("7"; "(^ ?x (* (! ?x) ?y))" => "(! (* (! ?x) (! ?y)))"),
        rw!("10"; "(! (* (! ?x) (! ?y)))" => "(^ ?x (* (! ?x) ?y))" ),
        rw!("8"; "(! (! ?y))" => "?y"),*/
    ];
    let mut cnt = rules.len();
    for line in rules_string.lines() {
        let mut split = line.split(";");
        let lhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        let rhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        rules.push(rw!({cnt.to_string()}; {lhs} => {rhs}));
        cnt += 1;
    }
    rules
}

fn egraph_from_seqn(innodes: &str, eqns: &str) -> (HashMap<String, Id>, EGraph<Prop, ()>) {
    let mut egraph = EGraph::<Prop, ()>::default();
    let mut ckt_node_to_eclass: HashMap<String, Id> = HashMap::new();
    for innode in innodes.split(" ") {
        //println!("{}", innode);
        let id = egraph.add(Prop::Symbol(Symbol::new(innode)));
        ckt_node_to_eclass.insert(innode.to_string(), id);
    }
    ckt_node_to_eclass.insert("true".to_string(), egraph.add(Prop::Bool(true)));
    ckt_node_to_eclass.insert("false".to_string(), egraph.add(Prop::Bool(false)));
    for eqn in eqns.lines() {
        let mut split = eqn.split("=");
        let lhs = split.next().unwrap();
        let mut rhs = split.next().unwrap().split(";");
        //dbg!(&rhs);
        // operator
        let op = rhs.next().unwrap();
        let src1 = ckt_node_to_eclass.get(rhs.next().unwrap());
        let src2 = ckt_node_to_eclass.get(rhs.next().unwrap());
        let id = match op {
            "^" => egraph.add(Prop::Xor([
                src1.unwrap().to_owned(),
                src2.unwrap().to_owned(),
            ])),
            "*" => egraph.add(Prop::And([
                src1.unwrap().to_owned(),
                src2.unwrap().to_owned(),
            ])),
            "!" => egraph.add(Prop::Not(src1.unwrap().to_owned())),
            "w" => src1.unwrap().to_owned(),
            _ => panic!("unrecognized op {}", op),
        };
        ckt_node_to_eclass.insert(lhs.to_string(), id);
    }
    (ckt_node_to_eclass, egraph)
}

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

impl<N: Analysis<Prop>> LpCostFunction<Prop, N> for MultComplexity {
    fn node_cost(&mut self, _egraph: &EGraph<Prop, N>, _eclass: Id, enode: &Prop) -> f64 {
        match enode {
            Prop::And(..) => 1.0,
            _ => 0.0,
        }
    }
}

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


#[derive(Clone,Debug)]
struct DepthArea {
    depth: usize,
    area: f64,
}
impl DepthArea {
    fn cost(&self) -> usize {
        self.depth*self.depth * (self.area as usize)
    }
    fn new() -> Self {
        DepthArea { depth: 0, area: 0.0 }
    }
    fn max() -> Self {
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

/*
pub struct MixedCost<'a, L: Language, N: Analysis<L>> {
    egraph: &'a EGraph<L,N>,
    enode_opt_lookup: HashMap<egraph_serialize::NodeId, f64>
}
impl <'a, N: Analysis<Prop>> egg::CostFunction<Prop> for MixedCost<'a, Prop, N> {
    type Cost = DepthArea;
    fn cost<C>(&mut self, enode: &Prop, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let mut base = enode.fold(DepthArea::new(), |sum, i| sum + costs(i));
        // TODO all of this is incredibly stupid
        let mut enode_m = RecExpr::default();
        enode_m.add(enode.clone());
        let eclass = self.egraph.lookup_expr(&enode_m).unwrap();
        let eclass = &self.egraph[eclass];
        let mut area_cost: f64 = INFINITY;
        for (i,n) in eclass.nodes.iter().enumerate() {
            if n == enode {
                let node_id_ser = NodeId::from(format!("{}.{}", eclass.id, i));
                let cost_lookup = self.enode_opt_lookup.get(&node_id_ser);
                if let Some(cost) = cost_lookup {
                    area_cost = *cost;
                    break;
                }
            }
        }
        match enode {
            Prop::And(_) => {base.area = area_cost; base.depth += 1;},
            _ => {}
        };
        base
    }
}*/

fn dag_md_traversal<'a, N>(
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
                    netd.push_str(if *b { "true;" } else { "false;" });
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

fn recexpr_traversal(expr: RecExpr<Prop>, outnodes: &str, outnode_ids: &Vec<Id>) -> String {
    let mut network: Vec<String> = Vec::new();

    for (id, p) in expr.as_ref().iter().enumerate() {
        let mut netd = format!("n{id} = ");
        match p {
            Prop::And([a, b]) => {
                netd.push_str(format!("n{} * n{};", a, b).as_str());
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
            _ => {}
        }
        network.push(netd);
    }

    for (o_id, o_name) in outnode_ids.iter().zip(outnodes.split(" ")) {
        network.push(format!("{} = n{};", o_name, o_id));
    }

    network.join("\n")
}

enum ExtractMode {
    MC,
    MD,
}

impl FromStr for ExtractMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mc" => Ok(Self::MC),
            "md" => Ok(Self::MD),
            _ => Err(()),
        }
    }
}

pub fn serialize_egraph<L, A>(egraph: &EGraph<L, A>, root_eclasses: &Vec<Id>) -> egraph_serialize::EGraph
where
    L: Language + Display,
    A: Analysis<L>,
{
    use egraph_serialize::*;
    let mut out = EGraph::default();
    for class in egraph.classes() {
        for (i, node) in class.nodes.iter().enumerate() {
            out.add_node(
                format!("{}.{}", class.id, i),
                Node {
                    op: node.to_string(),
                    children: node
                        .children()
                        .iter()
                        .map(|id| NodeId::from(format!("{}.0", id)))
                        .collect(),
                    eclass: ClassId::from(format!("{}", class.id)),
                    cost: Cost::new(1.0).unwrap(),
                    subsumed: false
                },
            )
        }
    }
    out.root_eclasses = root_eclasses.iter().map(|x| x.to_string().into()).collect();
    out
}


pub struct MixedCost<'a, L: Language, N: Analysis<L>> {
    egraph: &'a EGraph<L,N>,
    enode_opt_lookup: HashMap<egraph_serialize::NodeId, f64>,
    results: HashMap<Id, (usize, Prop)>,
    visited: HashMap<Id,Id>
}
impl <'a, N: Analysis<Prop>> MixedCost<'a, Prop, N> {
    fn select_best_eclass_mixed(&mut self, eclass: Id, depth: usize) -> usize {
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
                self.results.get(c).map(|x| { x.0}).unwrap_or_else(|| self.select_best_eclass_mixed(*c, depth+1))
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
            if node_cost < best_cost {
                best_cost = node_cost;
                best_prop = Some(node.clone());
            }
        }
        self.results.insert(eclass, (best_cost.depth, best_prop.unwrap()));
        return best_cost.depth;
    }
}

fn main() {
    env_logger::init();
    let mut args = std::env::args();
    args.next();
    let mode: ExtractMode = args
        .next()
        .expect("No mode supplied!")
        .parse()
        .expect("Invalid mode!");
    let timeout_seconds = args.next().expect("No timeout given").parse::<u64>().expect("Invalid timeout").max(60);
    let start_expr_path = args.next().expect("No input expr file given!");
    let rules_path = args.next().expect("No input rules file given!");
    let output_eqn_path = args.next().expect("No output path given!");

    let start_time = Instant::now();

    let rules_string = std::fs::read_to_string(rules_path).unwrap();
    let rules = process_rules(&rules_string);

    let start_string = std::fs::read_to_string(start_expr_path.clone()).unwrap();
    let mut start_lines = start_string.lines();
    let innodes = start_lines.next().unwrap();
    let outnodes = start_lines.next().unwrap();
    let start = start_lines.collect::<Vec<&str>>().join("\n");
    let (ckt_node_to_eclass, start_egraph) = egraph_from_seqn(innodes, start.as_str());

    let runner = Runner::default()
        .with_egraph(start_egraph)
        .with_time_limit(Duration::from_secs(timeout_seconds))
        .with_node_limit(250000000)
        .with_iter_limit(10000000)
        .run(rules.iter());
    let sat_time = Instant::now() - start_time;
    //println!("saturated {}", runner.egraph.classes().len());

    let mut outnode_ids: Vec<Id> = Vec::new();
    for outnode in outnodes.split(" ") {
        let id = ckt_node_to_eclass
            .get(outnode)
            .unwrap_or_else(|| panic!("no eclass matching output net {}", outnode));
        outnode_ids.push(runner.egraph.find(*id));
    }

    let egraph_ser = serialize_egraph(&runner.egraph, &outnode_ids,);

    for (k,_) in std::env::vars() {
        if k == "EGG_SERIALIZE" {
            egraph_ser.to_json_file("egraph.json").unwrap();
            break;
        }
    }

    let network = match mode {
        ExtractMode::MD => {
            let mc_optimal = global_greedy_dag::mc_extract(&egraph_ser, &egraph_ser.root_eclasses);
            let mut mixedcost = MixedCost {
                egraph: &runner.egraph,
                enode_opt_lookup: mc_optimal,
                results: HashMap::new(),
                visited: HashMap::new()
            };
            for outnode_id in outnode_ids.iter() {
                mixedcost.select_best_eclass_mixed(*outnode_id, 0);
            }
            //let extractor = Extractor::new(&runner.egraph, mixedcost);
            dag_md_traversal(&mixedcost, outnodes, &outnode_ids).1
        }
        ExtractMode::MC => {
            let mut extractor = LpExtractor::new(&runner.egraph, MultComplexity);
            extractor.timeout(300.0); // way too much time
            let (exp, outnode_ids) = extractor.solve_multiple(outnode_ids.as_slice());
            recexpr_traversal(exp, outnodes, &outnode_ids)
        }
    };
    let total_time = Instant::now() - start_time;
    println!("{},{},{}", start_expr_path, total_time.as_secs(), sat_time.as_secs());
    // output_eqn_path
    std::fs::write(output_eqn_path, format!(
        "INORDER = {};\nOUTORDER = {};\n{}",
        innodes, outnodes, network
    )).unwrap();
}
