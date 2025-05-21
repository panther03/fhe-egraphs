use clap::{Parser, Subcommand};
use egg::{rewrite as rw, *};
use egraph_serialize::NodeId;
use env_logger::init;
use extraction_unser::MultDepth;
use indexmap::IndexMap;
use rand::Rng;
use rand::SeedableRng;
use serde::deserialize_into_existing;
use std::collections::HashMap;
use std::collections::HashSet;
use std::f32::consts::E;
use std::io::Seek;
use std::io::Write;
use std::ops::Index;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod common;
mod extraction_ser;
mod extraction_unser;
mod global_greedy_dag;
mod md_mc_balanced_extract;
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

fn egraph_init_from_pis(innodes: &str) -> (EGraph<Prop, ()>, HashMap<String, Id>) {
    let mut egraph = EGraph::<Prop, ()>::default();

    let mut ckt_node_to_eclass: HashMap<String, Id> = HashMap::new();
    ckt_node_to_eclass.insert("true".to_string(), egraph.add(Prop::Bool(true)));
    ckt_node_to_eclass.insert("false".to_string(), egraph.add(Prop::Bool(false)));

    for innode in innodes.split(" ") {
        let id = egraph.add(Prop::Symbol(Symbol::new(innode)));
        ckt_node_to_eclass.insert(innode.to_string(), id);
    }
    (egraph, ckt_node_to_eclass)
}

fn egraph_from_seqn_trace(
    innodes: &str,
    outnodes: &str,
    trace: &str
) -> EqsatOptimizer {
    let (mut egraph, _) = egraph_init_from_pis(innodes);
    let num_pis = innodes.split(" ").count();

    let mut committed: Vec<&str> = Vec::new();
    let mut temp: Vec<&str> = Vec::new();
    for insn in trace.lines() {
        if insn == "COMMIT" {
            committed.append(&mut temp);
        } else if insn == "FORGET" {
            if !committed.is_empty() {
                temp.clear();
            }
        } else if insn.starts_with("COM") {
            continue;
        } else {
            temp.push(insn);
        }
    }
    committed.append(&mut temp);
    let mut pos: Vec<Id> = Vec::new();
    let mut index_map: HashMap<usize, Id> = HashMap::new();
    let mut prev_index_map: HashMap<usize, Id> = HashMap::new();
    // mapping to false e-class
    index_map.insert(0, Id::from(1 as usize));
    for i in 0..num_pis {
        index_map.insert(i + 1, Id::from(2 + i as usize));
    }
    let mut ind = 1;
    for insn_s in committed {
        let mut insn = insn_s.split(" ");
        let op = insn.next().unwrap();
        match op {
            "L" => {
                let old: usize = insn.next().unwrap().parse().unwrap();
                let compl: u32 = insn.next().unwrap().parse().unwrap();
                let new: usize = insn.next().unwrap().parse().unwrap();
                let new_n = if compl == 1 {
                    egraph.add(Prop::Not(prev_index_map[&old]))
                } else {
                    prev_index_map[&old]
                };
                index_map.insert(new, new_n);
            }
            "X" | "A" => {
                let n: usize = insn.next().unwrap().parse().unwrap();
                // don't re-add nodes and overwrite the id, because they might be aliased and structurally not equivalent
                if !index_map.contains_key(&n) {
                    let ac: u32 = insn.next().unwrap().parse().unwrap();
                    let a: usize = insn.next().unwrap().parse().unwrap();
                    let bc: u32 = insn.next().unwrap().parse().unwrap();
                    let b: usize = insn.next().unwrap().parse().unwrap();
                    let a = if ac == 1 {
                        egraph.add(Prop::Not(index_map[&a]))
                    } else {
                        index_map[&a]
                    };
                    let b = if bc == 1 {
                        egraph.add(Prop::Not(index_map[&b]))
                    } else {
                        index_map[&b]
                    };
                    let nid = if op == "X" {
                        egraph.add(Prop::Xor([a, b]))
                    } else {
                        egraph.add(Prop::And([a, b]))
                    };
                    index_map.insert(n, nid);
                }
            }
            "O" => {
                let ind: usize = insn.next().unwrap().parse().unwrap();
                let compl: u32 = insn.next().unwrap().parse().unwrap();
                let po_n: usize = insn.next().unwrap().parse().unwrap();
                let po_n = if compl == 1 {
                    egraph.add(Prop::Not(index_map[&po_n]))
                } else {
                    index_map[&po_n]
                };
                if pos.len() <= ind {
                    assert!(ind == pos.len());
                    pos.push(po_n);
                } else {
                    egraph.union(pos[ind], po_n);
                    pos[ind] = po_n;
                }
            }
            "U" => {
                let c: usize = insn.next().unwrap().parse().unwrap();
                let compl: u32 = insn.next().unwrap().parse().unwrap();
                let new_n: usize = insn.next().unwrap().parse().unwrap();
                let c = index_map[&c];
                let new_n = if compl == 1 {
                    egraph.add(Prop::Not(index_map[&new_n]))
                } else {
                    index_map[&new_n]
                };
                egraph.union(c, new_n);
            }
            "CLEAR" => {
                if index_map.len() != (num_pis + 1) {
                    prev_index_map = index_map.clone();
                }
                index_map.clear();
                index_map.insert(0, Id::from(1 as usize));
                for i in 0..num_pis {
                    index_map.insert(i + 1, Id::from(2 + i as usize));
                }
            }
            _ => {
                panic!("invalid op in insn: {}", op);
            }
        }
        ind += 1;
    }
    // re-canonicalize e-graph
    let egraph_c = egraph.clone(); 
    egraph.classes_mut().for_each(|c| {
        c.nodes.iter_mut().for_each(|n| {
            n.children_mut().iter_mut().for_each(|c| {
                *c = egraph_c.find(* c); 
            });
        });
    });
    for po in pos.iter_mut() {
        *po = egraph.find(*po);
    }
    let out_net_to_eclass: IndexMap<String, Id> = outnodes.split(" ").into_iter().enumerate().map(|(po_ind, po_net)| (po_net.to_string(), pos[po_ind])).collect();
    let concat_node = egraph.add(Prop::Concat(pos));
    EqsatOptimizer {
        egraph,
        rules: Vec::new(),
        concat_node,
        out_net_to_eclass,
        params: OptimizerParams::default(),
        stats: OptimizerStats::default().with_egraph_stats(&egraph_c)
    }
}

fn egraph_from_seqn(
    innodes: &str,
    outnodes: &str,
    eqns: &str,
) -> EqsatOptimizer {
    let (mut egraph, mut ckt_node_to_eclass) = egraph_init_from_pis(innodes);

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
        out_net_to_eclass.insert(outnode.to_string(),outnode_id);
    }

    /*egraph.add(Prop::Concat2([outnodes_vec[0], outnodes_vec[1]]));
    for n in &outnodes_vec[2..] {
        concat_node = egraph.add(Prop::Concat2([concat_node, *n]));
    }*/

    //let concat_node = egraph.add(Prop::Concat(outnodes_vec));
    EqsatOptimizer {
        egraph,
        concat_node: egg::Id::from(0 as usize),
        rules: Vec::new(),
        out_net_to_eclass,
        params: OptimizerParams::default(),
        stats: OptimizerStats::default()
    }
}

//////////////////////////
// Equality Saturation //
////////////////////////
#[derive(Clone, Default, Debug)]
struct OptimizerParams {
    time_limit: u64,
    node_limit: usize,
    iter_limit: usize,
    ilp_time_limit: f64,
    comm_matching: bool,
    strict_deadlines: bool,
}


struct OptimizerStats {
    final_eclasses: usize,
    final_enodes: usize,
    sat_time: Duration,
    extract_time: Duration,
}

impl Default for OptimizerStats {
    fn default() -> Self {
        Self {
            final_eclasses: 0,
            final_enodes: 0,
            sat_time: Duration::from_micros(0),
            extract_time: Duration::from_micros(0)
        }
    }
}

impl OptimizerStats {
    fn with_egraph_stats(mut self, egraph: &EGraph<Prop, ()>) -> Self {
        self.final_eclasses = egraph.number_of_classes();
        self.final_enodes = egraph.total_number_of_nodes();
        self
    }

    fn set_egraph_stats(&mut self, egraph: &EGraph<Prop, ()>) {
        self.final_eclasses = egraph.number_of_classes();
        self.final_enodes = egraph.total_number_of_nodes();
    }

    fn set_saturation_time(&mut self, time: Duration) {
        self.sat_time = time;
    }

    fn set_extraction_time(&mut self, time: Duration) {
        self.extract_time = time;
    }
}

struct EqsatOptimizer {
    egraph: EGraph<Prop, ()>,
    rules: Vec<Rewrite<Prop, ()>>,
    concat_node: Id,
    out_net_to_eclass: IndexMap<String, Id>,
    params: OptimizerParams,
    stats: OptimizerStats 
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
        let remapped = egraph.find(id);
        if remapped != id {
            continue;
        }
        if enter {
            *color.get_mut(&remapped).unwrap() = Color::Gray;
            stack.push((false, remapped));
            for (i, node) in egraph[remapped].iter().enumerate() {
                for child in node.children() {
                    match &color[&egraph.find(*child)] {
                        Color::White => stack.push((true, *child)),
                        Color::Gray => f(remapped, i),
                        Color::Black => (),
                    }
                }
            }
        } else {
            *color.get_mut(&remapped).unwrap() = Color::Black;
        }
    }
}

impl EqsatOptimizer {
    fn with_rules(mut self, rules: Vec<Rewrite<Prop, ()>>) -> Self {
        self.rules = rules;
        self
    }

    fn with_params(mut self, params: OptimizerParams) -> Self {
        self.params = params;
        self
    }

    fn saturate_egg (
        &mut self,
    ) {
        let start_time = Instant::now();
        dbg!(&self.params);
        let runner = Runner::default()
            .with_egraph(self.egraph.clone())
            .with_time_limit(Duration::from_secs(self.params.time_limit))
            .with_node_limit(self.params.node_limit)
            .with_iter_limit(self.params.iter_limit);
        //.with_scheduler(BackoffScheduler::default().with_initial_match_limit(100))

        let runner = if self.params.comm_matching {
            runner
        } else {
            runner.without_comm_matching()
        };
        let runner = if self.params.strict_deadlines {
            runner.with_strict_deadline()
        } else {
            runner
        };

        dbg!(self.rules.len());
        let runner = runner.run(self.rules.iter());

        // Remap output net IDs.
        for (_, id) in self.out_net_to_eclass.iter_mut() {
            *id = runner.egraph.find(*id);
        }
        self.concat_node = runner.egraph.find(self.concat_node);

        // Create mapping from new -> old based on saturation
        // PRECONDITION: new_egraph must have been created or cloned from old_egraph initially (otherwise find is meaningless)
        //if let Some(old_egraph) = old_egraph {
        //    self.new_to_old = HashMap::new();
        //    for class in old_egraph.classes() {
        //        self.new_to_old
        //            .insert(runner.egraph.find(class.id), class.id);
        //    }
        //}
        let sat_time = Instant::now() - start_time;

        self.stats.set_egraph_stats(&runner.egraph);
        self.stats.set_saturation_time(sat_time);
        self.egraph = runner.egraph;
    }

    fn mc_ilp_extract(&mut self, depth_bound: Option<usize>) -> Option<(u64, u64, String)> {
        let start_time = Instant::now();

        // extraction
        let mut extractor = LpExtractor::new(&self.egraph, extraction_unser::MultComplexity, &[self.concat_node], depth_bound);
        extractor.timeout(self.params.ilp_time_limit); // way too much time
        
        let Some((exp, _)) = extractor.solve_multiple(&[self.concat_node]) else { return None };
        let mc = exp.iter().filter(|p| match p {
            Prop::And(_) => true,
            _ => false
        }).count() as u64;

        let extract_time = Instant::now() - start_time;
        self.stats.set_extraction_time(extract_time);
        let (md, ntk) = extraction_unser::recexpr_traversal(exp, &self.out_net_to_eclass);
        Some((md, mc, ntk))
    }

    fn mc_md_dag(&mut self) -> (u64, u64, String) {
        let start_time = Instant::now();
        let egraph_ser = serde::serialize_in_mem(&self.egraph, self.out_net_to_eclass.values().into_iter());
        //let mut cycles: HashMap<NodeId, usize> = HashMap::new();
        //find_cycles(&self.egraph, |id, i| {
        //    let id: usize = id.into();
        //    let n = NodeId::new(i as u32, id as u32);
        //    dbg!(n);
        //    cycles.insert(n, 0);
        //});
        //extraction_ser::ser_egraph_to_dot::<&str>(&egraph_ser, &HashMap::new(), &cycles, "out.dot");

        let mut cost_analysis = global_greedy_dag::mc_extract(&egraph_ser, &egraph_ser.root_eclasses, HashMap::new());
        let extract_time = Instant::now() - start_time;
        self.stats.set_extraction_time(extract_time);
        extraction_ser::dag_network_writer(&egraph_ser, &mut cost_analysis, &self.out_net_to_eclass)
    }
}

//////////////////
// Main driver //
////////////////

//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand, PartialEq)]
enum FlowMode {
    SatMcIlp,
    SatMcMdDag,
    TracingHEConverge {
        #[arg(long)]
        ilp_iters: Option<usize>,
    },
}

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
    /// Trace file to construct e-graph
    #[arg(long)]
    trace: Option<PathBuf>,
    /// Timeout in seconds (per saturation iteration)
    #[arg(long)]
    egg_time_limit: Option<u64>,
    /// Number of e-graph iterations
    #[arg(long)]
    egg_iter_limit: Option<usize>,
    /// Max e-node count
    #[arg(long)]
    egg_node_limit: Option<usize>,
    /// Timeout in seconds for ILP
    ilp_time_limit: Option<f64>,

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
    let ilp_time_limit = args.ilp_time_limit.unwrap_or_else(|| {
        env_vars
            .get("EQSATOPT_ILP_TIME_LIMIT")
            .and_then(|x| x.parse::<f64>().ok())
            .unwrap_or(600.)
    });

    // Parse input network
    let infile = args.infile.as_path();
    let in_network = std::fs::read_to_string(infile).unwrap();
    let mut start_lines = in_network.lines();
    let innodes = start_lines.next().unwrap();
    let outnodes = start_lines.next().unwrap();
    let eqns = start_lines.collect::<Vec<&str>>().join("\n");

    let mut opter = if let Some(trace) = args.trace {
        let trace = std::fs::read_to_string(trace).unwrap();
        egraph_from_seqn_trace(innodes, outnodes, &trace)
    } else {
        egraph_from_seqn(innodes, outnodes, &eqns)
    }.with_rules(rules)
    .with_params(OptimizerParams {
        time_limit,
        node_limit,
        iter_limit,
        ilp_time_limit,
        comm_matching: !args.no_comm_matching,
        strict_deadlines: args.strict_deadlines,
    });

    let network = match args.flow {
        FlowMode::SatMcIlp => {
            opter.saturate_egg();
            println!("classes = {}; nodes = {}", opter.stats.final_eclasses, opter.stats.final_enodes);
            let (heur_md, heur_mc,ntk) = opter.mc_md_dag();
            println!("heur = ({},{})", heur_md, heur_mc);
            let ilp_result = opter.mc_ilp_extract(None);
            if let Some((ilp_md,ilp_mc,_)) = ilp_result {
                println!("ilp solution = ({},{})", ilp_md, ilp_mc);
            } else {
                println!("ilp timeout");
            }
            //opter.egraph.dot().to_png("egraph.png").unwrap();
            ntk
        }
        FlowMode::SatMcMdDag => {
            unimplemented!()
        }
        FlowMode::TracingHEConverge { ilp_iters } => {
            println!("classes = {}; nodes = {}", opter.stats.final_eclasses, opter.stats.final_enodes);
            let mut cycle_cnt = 0;
            find_cycles(&opter.egraph, |id, i| {
                //let node = opter.egraph.find(id);
                //let node = &opter.egraph[node];
                cycle_cnt += 1;
                //println!("cycle: {} {}", id, node);
            });
            println!("# of cycles: {}", cycle_cnt);
            let ilp_iters = ilp_iters.unwrap_or_else(|| {
                env_vars
                    .get("EQSATOPT_ILP_ITERS")
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(1)
            });
            let (best_md, heur_mc, mut best_ntk) = opter.mc_md_dag();
            let mut best_he_cost = best_md * best_md * heur_mc; 

            println!("Starting ILP HE exploration with MD = {}; MC = {}", best_md, heur_mc);
            for i in 0..ilp_iters+1 {
                let md_b = if i == 0 { None } else { Some(best_md as usize + (i-1)) };
                let Some((md, mc, ntk)) = opter.mc_ilp_extract(md_b) else { continue };
                if !md_b.is_none() && Some(md as usize) > md_b {
                    println!("WARNING: solution returned, but did not meet MD bounds - could be normal, continuing");
                    continue;
                }
                let he_cost = (md * md) as u64 * mc;
                if he_cost < best_he_cost {
                    best_he_cost = he_cost;
                    best_ntk = ntk;
                }
            }
            best_ntk
        }
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
