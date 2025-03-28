use clap::{Parser, Subcommand};
use egg::{rewrite as rw, *};
use egraph_serialize::NodeId;
use extraction_unser::MultDepth;
use indexmap::IndexMap;
use rand::Rng;
use rand::SeedableRng;
use serde::deserialize_into_existing;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Seek;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod common;
mod extraction_ser;
mod extraction_unser;
mod global_greedy_dag;
//mod md_mc_balanced_extract;
mod md_slack;
mod serde;
mod traverse;

use common::Prop;

///////////////////////////////////////
// Saturation setup (input parsing) //
/////////////////////////////////////

fn parse_rules(rules: &mut Vec<Rewrite<Prop, ()>>, rules_string: &str) {
    for line in rules_string.lines() {
        let mut split = line.split(":");
        let name = split.next().unwrap();
        let body = split.next();
        if body.is_none() {
            panic!(
                "malformed rule file: expected \"<name>:<lhs>[=>|<=>]<rhs>\"; got {}",
                line
            );
        }
        let body = body.unwrap();
        if body.contains("<=>") {
            let mut split = body.split("<=>");
            let lhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
            let rhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
            rules.extend(rw!({name}; {lhs.clone()} <=> {rhs.clone()}));
        } else if line.contains("=>") {
            let mut split = body.split("=>");
            let lhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
            let rhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
            rules.push(rw!({name}; {lhs} => {rhs}));
        } else {
            panic!(
                "malformed rule file: expected \"<name>:<lhs>[=>|<=>]<rhs>\"; got {}",
                line
            );
        }
    }
}

fn egraph_from_seqn(
    innodes: &str,
    outnodes: &str,
    eqns: &str,
    explanations_enabled: bool,
) -> (EGraph<Prop, ()>, IndexMap<String, Id>, Option<Id>) {
    let mut egraph = EGraph::<Prop, ()>::default();
    if explanations_enabled {
        egraph = egraph.with_explanations_enabled();
    }
    let mut ckt_node_to_eclass: HashMap<String, Id> = HashMap::new();
    for innode in innodes.split(" ") {
        //println!("{}", innode);
        let id = egraph.add(Prop::Symbol(Symbol::new(innode)));
        ckt_node_to_eclass.insert(innode.to_string(), id);
    }
    ckt_node_to_eclass.insert("true".to_string(), egraph.add(Prop::Bool(true)));
    ckt_node_to_eclass.insert("false".to_string(), egraph.add(Prop::Bool(false)));
    for (_, eqn) in eqns.lines().into_iter().enumerate() {
        let mut split = eqn.split("=");
        let lhs = split.next().unwrap();
        let mut rhs = split.next().unwrap().split(";");
        // operator
        let op = rhs.next().unwrap();
        let src1_s = rhs.next().unwrap();
        if let Ok(l1) = src1_s.parse::<u32>() {
            if ckt_node_to_eclass.get(src1_s).is_none() {
                ckt_node_to_eclass.insert(src1_s.to_string(), egraph.add(Prop::Int(l1)));
            }
        }

        let src2_s = rhs.next().unwrap();
        if let Ok(l2) = src2_s.parse::<u32>() {
            if ckt_node_to_eclass.get(src2_s).is_none() {
                ckt_node_to_eclass.insert(src2_s.to_string(), egraph.add(Prop::Int(l2)));
            }
        }

        let src1 = ckt_node_to_eclass.get(src1_s);
        let src2 = ckt_node_to_eclass.get(src2_s);
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
    let mut out_net_to_eclass: IndexMap<String, Id> = IndexMap::new();
    let mut outnodes_vec: Vec<Id> = Vec::new();
    for outnode in outnodes.split(" ") {
        let outnode_id = egraph.find(*ckt_node_to_eclass.get(outnode).unwrap());
        outnodes_vec.push(outnode_id);
        out_net_to_eclass.insert(outnode.to_string(), outnode_id);
    }
    let concat_id = if explanations_enabled {
        let mut concat_node = egraph.add(Prop::Concat2([outnodes_vec[0], outnodes_vec[1]]));
        for n in &outnodes_vec[2..] {
            concat_node = egraph.add(Prop::Concat2([concat_node, *n]));
        }
        // Some(egraph.add(Prop::Concat(outnodes_vec)))
        Some(concat_node)
    } else {
        None
    };
    (egraph, out_net_to_eclass, concat_id)
}

//////////////////////////
// Equality Saturation //
////////////////////////
#[derive(Clone)]
struct OptimizerParams {
    time_limit: u64,
    node_limit: usize,
    iter_limit: usize,
    comm_matching: bool,
    strict_deadlines: bool,
}

#[derive(Clone)]
struct EqsatOptimizer {
    rules: Vec<Rewrite<Prop, ()>>,
    out_net_to_eclass: IndexMap<String, Id>,
    new_to_old: HashMap<Id, Id>,
    params: OptimizerParams,
}

fn find_cycles<L, N>(egraph: &EGraph<L, N>, mut f: impl FnMut(Id, usize))
where
    L: Language,
    N: Analysis<L>,
{
    enum Color {
        White,
        Gray,
        Black,
    }
    type Enter = bool;

    let mut color: HashMap<Id, Color> = egraph.classes().map(|c| (c.id, Color::White)).collect();
    let mut stack: Vec<(Enter, Id)> = egraph.classes().map(|c| (true, c.id)).collect();

    while let Some((enter, id)) = stack.pop() {
        if enter {
            *color.get_mut(&id).unwrap() = Color::Gray;
            stack.push((false, id));
            for (i, node) in egraph[id].iter().enumerate() {
                for child in node.children() {
                    match &color[child] {
                        Color::White => stack.push((true, *child)),
                        Color::Gray => f(id, i),
                        Color::Black => (),
                    }
                }
            }
        } else {
            *color.get_mut(&id).unwrap() = Color::Black;
        }
    }
}

fn lock_in_random_nodes(
    egraph: &egraph_serialize::EGraph,
    cycle_classes: &HashSet<egraph_serialize::ClassId>,
    alpha: f64,
    seed: u64,
) -> HashMap<egraph_serialize::ClassId, NodeId> {
    let mut locked = HashMap::new();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    for (cid, class) in egraph.classes() {
        if !cycle_classes.contains(cid) && rng.random_range(0.0..1.0) > alpha {
            let ind = rng.random_range(0..class.nodes.len());
            locked.insert(*cid, class.nodes[ind]);
        }
    }
    locked
}

fn pool_extract(
    egraph: &egraph_serialize::EGraph,
    old_egraph: &egraph_serialize::EGraph,
    old_out_net_to_eclass: &IndexMap<String, Id>,
    out_net_to_eclass: &IndexMap<String, Id>,
    cycle_classes: &HashSet<egraph_serialize::ClassId>,
    num_candidates: usize,
    alpha: f64,
) -> (u64,String) {
    // TODO: actually 1st candidate should be the original network
    let mut cost_analysis =
        global_greedy_dag::mc_extract(old_egraph, &old_egraph.root_eclasses, HashMap::new());
    let (mut best_cost, mut best_network) =
        extraction_ser::dag_network_writer(old_egraph, &mut cost_analysis, old_out_net_to_eclass);

    for i in 0..num_candidates + 1 {
        let alpha = if i == 0 { 1.0 } else { alpha };
        let locked = lock_in_random_nodes(egraph, cycle_classes, alpha, i as u64);
        let mut cost_analysis =
            global_greedy_dag::mc_extract(egraph, &egraph.root_eclasses, locked);
        let (he_cost, ntk) =
            extraction_ser::dag_network_writer(egraph, &mut cost_analysis, out_net_to_eclass);

        //println!("Candidate {i}: HE cost {}", he_cost);
        if he_cost < best_cost {
            best_cost = he_cost;
            best_network = ntk;
        }
    }
    (best_cost, best_network)
}

impl EqsatOptimizer {
    fn new(
        rules: Vec<Rewrite<Prop, ()>>,
        out_net_to_eclass: IndexMap<String, Id>,
        params: OptimizerParams,
    ) -> Self {
        Self {
            rules,
            out_net_to_eclass,
            new_to_old: HashMap::new(),
            params,
        }
    }

    fn saturate(
        &mut self,
        new_egraph: EGraph<Prop, ()>,
        old_egraph: Option<&EGraph<Prop, ()>>,
        comm_matching_override: bool
    ) -> EGraph<Prop, ()> {
        let runner = Runner::default()
            .with_egraph(new_egraph)
            .with_time_limit(Duration::from_secs(self.params.time_limit))
            .with_node_limit(self.params.node_limit)
            .with_iter_limit(self.params.iter_limit);
        //.with_scheduler(BackoffScheduler::default().with_initial_match_limit(100))
        
        let runner = if comm_matching_override {
            runner
        } else {
            runner.without_comm_matching()
        };
        let runner = if self.params.strict_deadlines {
            runner.with_strict_deadline()
        } else {
            runner
        };

        let runner = runner.run(self.rules.iter());

        // Remap output net IDs.
        for (_, id) in self.out_net_to_eclass.iter_mut() {
            *id = runner.egraph.find(*id);
        }

        // Create mapping from new -> old based on saturation
        // PRECONDITION: new_egraph must have been created or cloned from old_egraph initially (otherwise find is meaningless)
        //if let Some(old_egraph) = old_egraph {
        //    self.new_to_old = HashMap::new();
        //    for class in old_egraph.classes() {
        //        self.new_to_old
        //            .insert(runner.egraph.find(class.id), class.id);
        //    }
        //}
        runner.egraph
    }

    /*

    fn mc_ilp_flow(mut self, initial_egraph: EGraph<Prop, ()>) -> (String, FlowStats) {
        let start_time = Instant::now();
        // saturation
        let sat_egraph = self.saturate(initial_egraph, None, true);
        let sat_time = Instant::now() - start_time;

        // extraction
        let mut extractor = LpExtractor::new(&sat_egraph, extraction_unser::MultComplexity);
        extractor.timeout(300.0); // way too much time
        let outnode_ids: Vec<Id> = self
            .out_net_to_eclass
            .values()
            .into_iter()
            .map(|x| *x)
            .collect();
        let (exp, expr_outnode_ids) = extractor.solve_multiple(outnode_ids.as_slice());
        let egraph_to_recexpr_ids =
            (outnode_ids.into_iter().zip(expr_outnode_ids.into_iter())).collect::<HashMap<_, _>>();
        self.out_net_to_eclass.iter_mut().for_each(|(_, v)| {
            *v = *egraph_to_recexpr_ids.get(v).unwrap_or(v);
        });
        let extract_time = Instant::now() - start_time - sat_time;
        (
            extraction_unser::recexpr_traversal(exp, &self.out_net_to_eclass),
            FlowStats {
                final_eclasses: sat_egraph.number_of_classes(),
                final_enodes: sat_egraph.total_number_of_nodes(),
                sat_time,
                extract_time,
            },
        )
    } */

    fn md_explain_flow(
        mut self,
        initial_egraph: EGraph<Prop, ()>,
        concat_node: Id,
    ) -> (String, FlowStats) {
        let start_time = Instant::now();
        // saturation
        let start_expr = initial_egraph.id_to_expr(concat_node);
        let mut sat_egraph = self.saturate(initial_egraph, None, true);
        let sat_time = Instant::now() - start_time;

        // extraction
        let extractor = Extractor::new(&sat_egraph, extraction_unser::MultDepth);
        let (_, best_node) = extractor.find_best(sat_egraph.find(concat_node));
        let explanation = sat_egraph.explain_equivalence(&start_expr, &best_node);
        println!("{}", explanation.get_string());
        let extract_time = Instant::now() - start_time - sat_time;

        (
            extraction_unser::recexpr_traversal(best_node, &self.out_net_to_eclass),
            FlowStats {
                final_eclasses: sat_egraph.number_of_classes(),
                final_enodes: sat_egraph.total_number_of_nodes(),
                sat_time,
                extract_time,
            },
        )
    }

    fn md_vanilla_flow(
        mut self,
        initial_egraph: EGraph<Prop, ()>,
        concat_node: Id,
    ) -> (String, FlowStats) {
        let start_time = Instant::now();
        // saturation
        let sat_egraph = self.saturate(initial_egraph, None, true);
        let sat_time = Instant::now() - start_time;

        // extraction
        let extractor = Extractor::new(&sat_egraph, egg::AstDepth);
        let (_, best_node) = extractor.find_best(sat_egraph.find(concat_node));
        let extract_time = Instant::now() - start_time - sat_time;

        (
            extraction_unser::recexpr_traversal(best_node, &self.out_net_to_eclass),
            FlowStats {
                final_eclasses: sat_egraph.number_of_classes(),
                final_enodes: sat_egraph.total_number_of_nodes(),
                sat_time,
                extract_time,
            },
        )
    }

    /*fn md_dag_flow(self, initial_egraph: EGraph<Prop, ()>) -> (String,FlowStats) {
        let start_time = Instant::now();
        // saturation
        let sat_egraph= self.saturate(initial_egraph, None);
        let sat_time = Instant::now() - start_time;

        let egraph_ser = serde::serialize_in_mem(&sat_egraph, &outnode_ids);
        //for (k,_) in std::env::vars() {
        //    if k == "EGG_SERIALIZE" {
        //
        //        break;
        //    }
        //}
        let mc_optimal = global_greedy_dag::mc_extract(&egraph_ser, &egraph_ser.root_eclasses);
        let mut mixedcost = extraction_unser::MixedCost {
            egraph: &sat_egraph,
            enode_opt_lookup: mc_optimal,
            results: HashMap::new(),
            visited: HashMap::new()
        };

        for outnode_id in outnode_ids.iter() {
            mixedcost.select_best_eclass_mixed(*outnode_id, 0);
        }
        let (_, network) = dag_md_traversal(&mixedcost, &self.outnodes, &outnode_ids);
        let extract_time = Instant::now() - start_time - sat_time;

        (network, FlowStats {
            final_eclasses: sat_egraph.number_of_classes(),
            final_enodes: sat_egraph.total_number_of_nodes(),
            sat_time,
            extract_time
        })
    }*/

    fn md_multiple_iters(
        mut self,
        initial_egraph: EGraph<Prop, ()>,
        iters: usize,
        alpha: f64,
        num_candidates: usize,
        comm_matching_override: bool,
    ) -> (String, FlowStats, u64) {
        let mut iter_initial_egraph = initial_egraph;
        let mut sat_time: Duration = Duration::from_secs(0);
        let mut extract_time: Duration = Duration::from_secs(0);
        let mut network: String = String::new();
        let mut he_cost = 0;
        for i in 0..iters {
            let iter_init_outnode_ids: IndexMap<String, Id> = self
                .out_net_to_eclass
                .iter()
                .map(|(net, v)| (net.clone(), *v))
                .collect();
            //iter_initial_egraph.dot().to_svg(format!("iter{}.svg", i)).unwrap();
            let start_time = Instant::now();
            // saturate
            let sat_egraph = self.saturate(iter_initial_egraph.clone(), Some(&iter_initial_egraph), comm_matching_override);
            let sat_time_iter = Instant::now() - start_time;
            sat_time += sat_time_iter;

            //let outnode_ids: Vec<Id> = self.out_net_to_eclass.values().map(|v| *v).collect();
            //dbg!(outnode_ids);
            //let concat_node = sat_egraph.add(Prop::Concat(outnode_ids));
            //let extractor = Extractor::new(&sat_egraph, MultDepth);
            //let (md, _) = extractor.find_best(sat_egraph.find(concat_node));
            //dbg!(md);
            let mut cycle_nodes: HashSet<egraph_serialize::ClassId> = HashSet::new();
            find_cycles(&sat_egraph, |id, _| {
                cycle_nodes.insert(egraph_serialize::ClassId::new(id.into()));
            });

            //dbg!("sat complete");

            //sat_egraph.dot().to_dot("egraph.dot");
            // convert to serialized graph
            //let mut egg_file = std::fs::File::create("out.egg").unwrap();
            //let mut egg_file: std::fs::File = tempfile::tempfile().unwrap();
            let egraph_ser = serde::serialize_in_mem(&sat_egraph, self.out_net_to_eclass.values().into_iter());
            //serde::serialize_to_binfile(
            //    &sat_egraph,
            //    self.out_net_to_eclass.values().into_iter(),
            //    &mut egg_file,
            //    |p| match p {
            //        Prop::And(_) => 1.0,
            //        _ => 0.0,
            //    },
            //)
            //.unwrap();
            //dbg!(sat_egraph.total_number_of_nodes());
            //dbg!(sat_egraph.number_of_classes());
            // 2 egraphs in memory at same time is bad
            std::mem::drop(sat_egraph);
            //dbg!("drop complete");
            //egg_file.seek(std::io::SeekFrom::Start(0)).unwrap();
            //let egg_file = std::fs::File::open("out.egg").unwrap();
            //let egraph_ser = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();

            // last iteration dont care about updating initial
            if i < iters - 1 {
                dbg!("start extract");
                let (slack, ckt_md, ckt_remaining) =
                    md_slack::calc_remaining(&egraph_ser, &egraph_ser.root_eclasses);
                //for (_, old) in self.out_net_to_eclass.iter() {
                //    dbg!(old);
                //    dbg!(ckt_remaining.get(&egraph_serialize::ClassId::new((*old).into())));
                //    dbg!(slack.md_lookup.get(&egraph_serialize::ClassId::new((*old).into())));
                //}
                dbg!("calc remaining");
                let (ser_to_unser, pruned_egraph) = md_slack::egraph_prune(
                    &egraph_ser,
                    &egraph_ser.root_eclasses,
                    &slack,
                    ckt_md,
                    &ckt_remaining,
                );
                iter_initial_egraph = pruned_egraph;
                //let prune_set = md_slack::egraph_prune_set(&egraph_ser, &egraph_ser.root_eclasses, &slack, ckt_md, &ckt_remaining);
                //let annot: HashMap<NodeId, String> = egraph_ser.nodes.iter().map(|(nid, _)| {
                //    (
                //        *nid,
                //        if ser_to_unser.contains_key(&egg::Id::from(nid.class() as usize)) {"NP:"} else {"P:"}.to_owned() + if prune_set.contains(nid) {"RP"} else {"RNP"}
                //    )
                //}).collect();
                //extraction_ser::ser_egraph_to_dot(&egraph_ser, &annot, format!("egraph{}.dot", i).as_str());

                //dbg!(iter_initial_egraph.number_of_classes());
                //dbg!(iter_initial_egraph.total_number_of_nodes());
                // remap the output nodes back to this original graph
                for (_, old) in self.out_net_to_eclass.iter_mut() {
                    *old = *ser_to_unser.get(old).unwrap();
                }
                dbg!("finish extract");
            } else {
                //dbg!("start final extract");
                let old_egraph = serde::serialize_in_mem(
                    &iter_initial_egraph,
                    iter_init_outnode_ids.values().into_iter(),
                );
                (he_cost, network) = pool_extract(
                    &egraph_ser,
                    &old_egraph,
                    &iter_init_outnode_ids,
                    &self.out_net_to_eclass,
                    &cycle_nodes,
                    num_candidates,
                    alpha,
                );
                //dbg!("finish final extract");
            }
            extract_time += Instant::now() - start_time - sat_time_iter;
        }
        (
            network,
            FlowStats {
                final_eclasses: 0,
                final_enodes: 0,
                sat_time,
                extract_time,
            },
            he_cost
        )
    }

    fn empty_flow(mut self, initial_egraph: EGraph<Prop, ()>,) -> String {
        let egraph_ser = serde::serialize_in_mem(&initial_egraph, self.out_net_to_eclass.values().into_iter());
        let mut cost_analysis =
        global_greedy_dag::mc_extract(&egraph_ser, &egraph_ser.root_eclasses, HashMap::new());
        let (mut best_cost, mut best_network) =
        extraction_ser::dag_network_writer(&egraph_ser, &mut cost_analysis, &self.out_net_to_eclass);
        best_network
    }
}

//////////////////
// Main driver //
////////////////
struct FlowStats {
    final_eclasses: usize,
    final_enodes: usize,
    sat_time: Duration,
    extract_time: Duration,
}

//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand, PartialEq)]
enum FlowMode {
    McIlp,
    MdExplain,
    MdDag,
    MdMultipleIters {
        #[arg(long)]
        iters: Option<usize>,
        #[arg(long)]
        alpha: Option<f64>,
        #[arg(long)]
        num_candidates: Option<usize>,
    },
    MdVanillaFlow,
    EmptyFlow,
}

//impl FromStr for FlowMode {
//    type Err = ();
//
//    fn from_str(s: &str) -> Result<Self, Self::Err> {
//        match s {
//            "mc-ilp" => Ok(Self::McIlp),
//            "md-explain" => Ok(Self::MdExplain),
//            "md-dag" => Ok(Self::MdDag),
//            "md-multiple-iters" => Ok(Self::MdMultipleIters),
//            "md-vanilla-flow" => Ok(Self::MdVanillaFlow),
//            _ => Err(()),
//        }
//    }
//}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    flow: FlowMode,
    /// Input logic network
    infile: PathBuf,
    /// Output path
    outfile: PathBuf,
    /// Rewriting rules (can specify multiple)
    #[arg(long)]
    rules: Vec<PathBuf>,
    /// Timeout in seconds (per saturation iteration)
    #[arg(long)]
    egg_time_limit: Option<u64>,
    /// Number of e-graph iterations
    #[arg(long)]
    egg_iter_limit: Option<usize>,
    /// Max e-node count
    #[arg(long)]
    egg_node_limit: Option<usize>,

    #[arg(long, action=clap::ArgAction::SetTrue)]
    no_comm_matching: bool,

    #[arg(long, action=clap::ArgAction::SetTrue)]
    strict_deadlines: bool,
}

fn main() {
    env_logger::init();

    let args = Args::parse();

    // Parse rules
    let mut rules: Vec<Rewrite<Prop, ()>> = Vec::new();
    for rules_f in args.rules {
        let rules_s = std::fs::read_to_string(rules_f).unwrap();
        parse_rules(&mut rules, &rules_s);
    }

    // Options
    let env_vars: HashMap<String, String> = std::env::vars().collect();

    let time_limit = args.egg_time_limit.unwrap_or_else(|| {
        env_vars
            .get("EQSATOPT_EGG_TIME_LIMIT")
            .and_then(|x| x.parse::<u64>().ok())
            .unwrap_or(60)
    });
    let iter_limit = args.egg_iter_limit.unwrap_or_else(|| {
        env_vars
            .get("EQSATOPT_EGG_ITER_LIMIT")
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(10000000)
    });
    let node_limit = args.egg_node_limit.unwrap_or_else(|| {
        env_vars
            .get("EQSATOPT_EGG_NODE_LIMIT")
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(250000000)
    });

    // Parse input network
    let infile = args.infile.as_path();
    let in_network = std::fs::read_to_string(infile).unwrap();
    let mut start_lines = in_network.lines();
    let innodes = start_lines.next().unwrap();
    let outnodes = start_lines.next().unwrap();
    let (start_egraph, out_net_to_eclass, concat_node) =
        if infile.extension().map(|ext| ext == "seqn").unwrap_or(false) {
            let start = start_lines.collect::<Vec<&str>>().join("\n");
            egraph_from_seqn(
                innodes,
                outnodes,
                start.as_str(),
                args.flow == FlowMode::MdExplain || args.flow == FlowMode::MdVanillaFlow,
            )
        }
        /* else if args.infile.ends_with(".sexpr") {
            let mut start_lines = in_network.lines();
            start_lines.next().unwrap();
            start_lines.next().unwrap();

            let sexpr = start.parse().unwrap();
            let mut start_egraph = EGraph::default();
            let concat_node = Some(start_egraph.add_expr(&sexpr));
        }*/
        else {
            panic!("unrecognied file extension for input")
        };

    //let opter = EqsatOptimizer::new(rules, out_net_to_eclass).with_timeout(timeout_seconds);
    let mut opter = EqsatOptimizer::new(
        rules,
        out_net_to_eclass,
        OptimizerParams {
            time_limit,
            node_limit,
            iter_limit,
            comm_matching: !args.no_comm_matching,
            strict_deadlines: args.strict_deadlines,
        },
    );

    let (network, stats) = match args.flow {
        FlowMode::McIlp => unimplemented!(),
        FlowMode::MdExplain => opter.md_explain_flow(start_egraph, concat_node.unwrap()),
        FlowMode::MdDag => unimplemented!(), //opter.md_dag_flow(start_egraph),
        FlowMode::MdMultipleIters {
            iters,
            alpha,
            num_candidates,
        } => {
            let iters = iters.unwrap_or_else(|| {
                env_vars
                    .get("EQSATOPT_CHECKPOINT_ITER")
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(10)
            });
            let alpha = alpha.unwrap_or_else(|| {
                env_vars
                    .get("EQSATOPT_POOL_ALPHA")
                    .and_then(|x| x.parse::<f64>().ok())
                    .unwrap_or(1.0)
            });
            let num_candidates = num_candidates.unwrap_or_else(|| {
                env_vars
                    .get("EQSATOPT_POOL_CANDIDATES")
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(1)
            });
            let (network_c, stats_c, he_cost_c) = opter.md_multiple_iters(start_egraph.clone(), iters, alpha, num_candidates, true);
            //let (network_nc, stats_nc, he_cost_nc) = opter.md_multiple_iters(start_egraph, iters, alpha, num_candidates, false);
            (network_c, stats_c)
            //if he_cost_c < he_cost_nc {
            //    (network_c, stats_c)
            //} else {
            //    (network_nc, stats_nc)
            //}
        }
        FlowMode::MdVanillaFlow => opter.md_vanilla_flow(start_egraph, concat_node.unwrap()),
        FlowMode::EmptyFlow => (opter.empty_flow(start_egraph),FlowStats {
            extract_time: Duration::from_micros(0),
            final_eclasses: 0,
            final_enodes: 0,
            sat_time: Duration::from_micros(0)
        }),
    };

    //println!(
    //    "{},{},{},{},{}",
    //    infile.display(),
    //    stats.sat_time.as_secs(),
    //    stats.extract_time.as_secs(),
    //    stats.final_eclasses,
    //    stats.final_enodes
    //);
    std::fs::write(
        args.outfile,
        format!(
            "INORDER = {};\nOUTORDER = {};\n{}",
            innodes, outnodes, network
        ),
    )
    .unwrap();
}
