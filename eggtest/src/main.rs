use egg::{rewrite as rw, *};
use std::cmp::Ordering;
use std::{collections::HashMap, str::FromStr};
use std::time::Duration;

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
    ];
    let mut cnt = 2;
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
    area: usize,
}
impl DepthArea {
    fn cost(&self) -> usize {
        self.depth*self.depth * self.area
    }
    fn new() -> Self {
        DepthArea { depth: 0, area: 0 }
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
        self.cost().eq(&other.cost())
    }
}
impl PartialOrd for DepthArea {
    fn partial_cmp(&self, other: &DepthArea) -> Option<Ordering> {
        self.cost().partial_cmp(&other.cost())
    }
}
pub struct MixedCost;
impl egg::CostFunction<Prop> for MixedCost {
    type Cost = DepthArea;
    fn cost<C>(&mut self, enode: &Prop, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let mut base = enode.fold(DepthArea::new(), |sum, i| sum + costs(i));
        match enode {
            Prop::And(_) => {base.area += 1; base.depth += 1;},
            _ => {}
        };
        base
    }
}

fn greedy_dag_extract<'a, CF, N>(
    extractor: &'a Extractor<'a, CF, Prop, N>,
    outnodes: &str,
    outnode_ids: &Vec<Id>,
) -> String
where
    CF: CostFunction<Prop>,
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

    for o_id in outnode_ids {
        todo_nodes.push(*o_id);
        todo_finishes.push(false);
    }

    while !todo_nodes.is_empty() {
        let eclass = todo_nodes.pop().unwrap();
        let mut netd = format!("n{} = ", eclass);

        // number of children this node introduces
        // may not be fixed if the eclasses have already been visited
        let mut new_children = 0;
        let already_added = eclass_seen.get(&eclass).is_some();
        if !already_added {
            let enode = extractor.find_best_node(eclass);
            eclass_seen.insert(eclass, eclass);
            match enode {
                Prop::And([a, b]) => {
                    netd.push_str(format!("n{} * n{};", a, b).as_str());

                    if eclass_seen.get(&a).is_none() {
                        todo_nodes.push(*a);
                        new_children += 1;
                    }
                    if eclass_seen.get(&b).is_none() {
                        todo_nodes.push(*b);
                        new_children += 1;
                    }
                }
                Prop::Xor([a, b]) => {
                    netd.push_str(format!("(!n{} * n{}) + (n{} * !n{});", a, b, a, b).as_str());

                    if eclass_seen.get(&a).is_none() {
                        todo_nodes.push(*a);
                        new_children += 1;
                    }
                    if eclass_seen.get(&b).is_none() {
                        todo_nodes.push(*b);
                        new_children += 1;
                    }
                }
                Prop::Not(a) => {
                    netd.push_str(format!("!n{};", a).as_str());

                    if eclass_seen.get(&a).is_none() {
                        todo_nodes.push(*a);
                        new_children += 1;
                    }
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
        }
        // "leaf" node
        // either an actual leaf or all of its children were visited already
        // or the node itself was already visited
        if new_children == 0 {
            if !already_added {
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

    for (o_id, o_name) in outnode_ids.iter().zip(outnodes.split(" ")) {
        real_network.push(format!("{} = n{};", o_name, o_id));
    }

    real_network.join("\n")
}

fn extract2(expr: RecExpr<Prop>, outnodes: &str, outnode_ids: &Vec<Id>) -> String {
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

fn main() {
    env_logger::init();
    let mode: ExtractMode = std::env::args()
        .nth(1)
        .expect("No mode supplied!")
        .parse()
        .expect("Invalid mode!");
    let start_expr_path = std::env::args().nth(2).expect("No input expr file given!");
    let rules_path = std::env::args().nth(3).expect("No input rules file given!");

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
        .with_time_limit(Duration::from_secs(30))
        .with_node_limit(100000000)
        .run(rules.iter());
    dbg!("extract {}", &start_expr_path);

    let mut outnode_ids: Vec<Id> = Vec::new();
    for outnode in outnodes.split(" ") {
        let id = ckt_node_to_eclass
            .get(outnode)
            .unwrap_or_else(|| panic!("no eclass matching output net {}", outnode));
        outnode_ids.push(*id);
    }

    let network = match mode {
        ExtractMode::MD => {
            let extractor = Extractor::new(&runner.egraph, MultComplexity);
            greedy_dag_extract(&extractor, outnodes, &outnode_ids)
        }
        ExtractMode::MC => {
            let mut extractor = LpExtractor::new(&runner.egraph, AstSize);
            extractor.timeout(100000.0); // way too much time
            let (exp, outnode_ids) = extractor.solve_multiple(outnode_ids.as_slice());
            extract2(exp, outnodes, &outnode_ids)
        }
    };

    println!(
        "INORDER = {};\nOUTORDER = {};\n{}",
        innodes, outnodes, network
    );
    dbg!("write {}", &start_expr_path);
}
