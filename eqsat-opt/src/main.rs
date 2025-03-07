use egg::{rewrite as rw, *};
use extraction_unser::dag_md_traversal;
use indexmap::IndexMap;
use serde::deserialize_into_existing;
use std::collections::HashSet;
use std::io::Seek;
use std::path::PathBuf;
use std::{collections::HashMap, str::FromStr};
use std::time::{Duration, Instant};

mod common;
mod global_greedy_dag;
mod extraction_unser;
mod extraction_ser;
mod serde;

use common::Prop;

///////////////////////////////////////
// Saturation setup (input parsing) //
/////////////////////////////////////

fn esyn_rules() -> Vec<Rewrite<Prop,()>> {
    let mut rules: Vec<Rewrite<Prop, ()>> = vec![
        // Hardcoded laws
        //rw!("commX"; "(^ ?x ?y)" => "(^ ?y ?x)"),
        //rw!("commA"; "(* ?x ?y)" => "(* ?y ?x)"),
        
        rw!("null-element1"; "(* ?b 0)" => "0"), 
        rw!("null-element2"; "(+ ?b 1)" => "1"), 
        rw!("complements1"; "(* ?b (! ?b))" => "0"), 
        rw!("complements2"; "(+ ?b (! ?b))" => "1"), 
        rw!("covering1"; "(* ?b (+ ?b ?c))" => "?b"), 
        rw!("covering2"; "(+ ?b (* ?b ?c))" => "?b"), 
        rw!("combining1"; "(+ (* ?b ?c) (* ?b (! ?c)))" => "?b"), 
        rw!("combining2"; "(* (+ ?b ?c) (+ ?b (! ?c)))" => "?b"),

        //// The following are all boolean only
        ////rw!("ident"; "(?y)" => "(! (! ?y))"),
        ////rw!("xorDef"; "(! (* (! (* ?x (! ?y))) (! (* (! ?x) ?y))))" => "(^ ?y ?x)"),
        ////rw!("bool1"; "(^ ?x (* ?x ?y))" => "(* ?x (! ?y))"),
        ////rw!("bool2"; "(^ ?x (* (! ?x) ?y))" => "(! (* (! ?x) (! ?y)))"),
        ////rw!("bool3"; "(! (* (! ?x) (! ?y)))" => "(^ ?x (* (! ?x) ?y))" ),
    ];
    rules.extend(rewrite!("identity1"; "(* ?b 1)" <=> "?b"));
    rules.extend(rewrite!("identity2'"; "(+ ?b 0)" <=> "?b"));
    rules.extend(rewrite!("idempotency1"; "(* ?b ?b)" <=> "?b"));
    rules.extend(rewrite!("idempotency2"; "(+ ?b ?b)" <=> "?b"));
    rules.extend(rewrite!("involution1"; "(! (! ?b))" <=> "?b"));
    rules.extend(rewrite!("commutativity1"; "(* ?b ?c)" <=> "(* ?c ?b)"));
    rules.extend(rewrite!("commutativity2"; "(+ ?b ?c)" <=> "(+ ?c ?b)"));
    rules.extend(rewrite!("associativity1"; "(*(* ?b ?c) ?d)" <=> "(* ?b (* ?c ?d))"));
    rules.extend(rewrite!("associativity2"; "(+(+ ?b ?c) ?d)" <=> "(+ ?b (+ ?c ?d))"));
    rules.extend(rewrite!("distributivity1"; "(+ (* ?b ?c) (* ?b ?d))" <=> "(* ?b (+ ?c ?d))"));
    rules.extend(rewrite!("distributivity2"; "(* (+ ?b ?c) (+ ?b ?d))" <=> "(+ ?b (* ?c ?d))"));
    rules.extend(rewrite!("consensus1"; "(+ (+ (* ?b ?c) (* (! ?b) ?d)) (* ?c ?d))" <=> "(+ (* ?b ?c) (* (! ?b) ?d))"));
    rules.extend(rewrite!("consensus2"; "(* (* (+ ?b ?c) (+ (! ?b) ?d)) (+ ?c ?d))" <=> "(* (+ ?b ?c) (+ (! ?b) ?d))"));
    rules.extend(rewrite!("de-morgan1"; "(! (* ?b ?c))" <=> "(+ (! ?b) (! ?c))"));
    rules.extend(rewrite!("de-morgan2"; "(! (+ ?b ?c))" <=> "(* (! ?b) (! ?c))"));
    rules
}

fn integer_rules() -> Vec<Rewrite<Prop,()>> {
    let mut rules: Vec<Rewrite<Prop, ()>> = Vec::new();
    rules.extend(rewrite!("assocA"; "(* ?x (* ?y ?z))" <=> "(* (* ?x ?y) ?z)"));
    rules.extend(rewrite!("assocX"; "(^ ?x (^ ?y ?z))" <=> "(^ (^ ?x ?y) ?z)"));
    rules.extend(rewrite!("factorDistrib"; "(^ (* ?x ?y) (* ?x ?z))" <=> "(* ?x (^ ?y ?z))"));
    rules
}

fn boolean_rules() -> Vec<Rewrite<Prop, ()>> {
    let mut rules: Vec<Rewrite<Prop, ()>> = Vec::new();
    rules.extend(rewrite!("assocA"; "(* ?x (* ?y ?z))" <=> "(* (* ?x ?y) ?z)"));
    rules.extend(rewrite!("assocX"; "(^ ?x (^ ?y ?z))" <=> "(^ (^ ?x ?y) ?z)"));
    rules.extend(rewrite!("factorDistrib"; "(^ (* ?x ?y) (* ?x ?z))" <=> "(* ?x (^ ?y ?z))"));
    rules.extend(rewrite!("manual4";"(^ ?x (* ?x ?y))" <=> "(* ?x (! ?y))"));
    rules.extend(rewrite!("manual5";"(^ ?x (* (! ?x) ?y))" <=> "(! (* (! ?x) (! ?y)))"));
    rules.extend(rewrite!("manual6";"(! (* (! ?x) (! ?y)))" <=> "(^ ?x (* (! ?x) ?y))"));
    rules
}

fn process_rules(rules_string: &str) -> Vec<Rewrite<Prop, ()>> {
    //let mut rules = esyn_rules();    
    let mut rules = boolean_rules();

    let mut cnt = 0;

    for line in rules_string.lines() {
        let mut split = line.split(";");
        let lhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        let rhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        rules.push(rw!({cnt.to_string()}; {lhs} => {rhs}));
        cnt += 1;
    }
    rules
}

fn egraph_from_seqn(innodes: &str, outnodes: &str, eqns: &str, explanations_enabled: bool) -> (EGraph<Prop, ()>, IndexMap<String, Id>, Option<Id>) {
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
    for (_,eqn) in eqns.lines().into_iter().enumerate() {
        let mut split = eqn.split("=");
        let lhs = split.next().unwrap();
        let mut rhs = split.next().unwrap().split(";");
        //dbg!(&rhs);
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
    let concat_id = if explanations_enabled { Some(egraph.add(Prop::Concat(outnodes_vec))) } else { None };
    (egraph, out_net_to_eclass, concat_id)
}


//////////////////////////
// Equality Saturation //
////////////////////////
struct EqsatOptimizer {
    rules: Vec<Rewrite<Prop, ()>>,
    out_net_to_eclass: IndexMap<String, Id>,
    new_to_old: HashMap<Id, Id>,
    timeout_secs: u64 
}

impl EqsatOptimizer {
    fn new(rules: Vec<Rewrite<Prop, ()>>, out_net_to_eclass: IndexMap<String, Id>) -> Self {
        Self {
            rules,
            out_net_to_eclass,
            new_to_old: HashMap::new(),
            timeout_secs: 60
        }
    }

    fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    fn saturate(&mut self, new_egraph: EGraph<Prop, ()>, old_egraph: Option<&EGraph<Prop, ()>>) -> EGraph<Prop, ()> {
        let runner = Runner::default()
            .with_egraph(new_egraph)
            .with_time_limit(Duration::from_secs(self.timeout_secs))
            .with_node_limit(250000000)
            .with_iter_limit(10000000)
            //.with_scheduler(BackoffScheduler::default().with_initial_match_limit(100))
            .run(self.rules.iter());
        
        // Remap output net IDs.
        for (_, id) in self.out_net_to_eclass.iter_mut() {
            *id = runner.egraph.find(*id);
        }

        // Create mapping from new -> old based on saturation
        // PRECONDITION: new_egraph must have been created or cloned from old_egraph initially (otherwise find is meaningless)
        if let Some(old_egraph) = old_egraph {
            self.new_to_old = HashMap::new();
            for class in old_egraph.classes() {
                self.new_to_old.insert(runner.egraph.find(class.id), class.id);
            }
        }
        runner.egraph
    }

    fn mc_ilp_flow(mut self, initial_egraph: EGraph<Prop, ()>) -> (String,FlowStats) {
        let start_time = Instant::now();
        // saturation
        let sat_egraph = self.saturate(initial_egraph, None);
        let sat_time = Instant::now() - start_time;
    
        // extraction
        let mut extractor = LpExtractor::new(&sat_egraph, extraction_unser::MultComplexity);
        extractor.timeout(300.0); // way too much time
        let outnode_ids: Vec<Id> = self.out_net_to_eclass.values().into_iter().map(|x| *x).collect();
        let (exp, expr_outnode_ids) = extractor.solve_multiple(outnode_ids.as_slice());
        let egraph_to_recexpr_ids = (outnode_ids.into_iter().zip(expr_outnode_ids.into_iter())).collect::<HashMap<_,_>>();
        self.out_net_to_eclass.iter_mut().for_each(|(_,v)| { *v = *egraph_to_recexpr_ids.get(v).unwrap_or(v); });
        let extract_time = Instant::now() - start_time - sat_time;
    
        (extraction_unser::recexpr_traversal(exp, &self.out_net_to_eclass), FlowStats {
            final_eclasses: sat_egraph.number_of_classes(),
            final_enodes: sat_egraph.total_number_of_nodes(),
            sat_time,
            extract_time
        })
    }
    
    fn md_explain_flow(mut self, initial_egraph: EGraph<Prop, ()>, concat_node: Id) -> (String,FlowStats) { 
        let start_time = Instant::now();
        // saturation
        let start_expr = initial_egraph.id_to_expr(concat_node);
        let mut sat_egraph = self.saturate(initial_egraph, None);
        let sat_time = Instant::now() - start_time;
    
        // extraction
        let extractor = Extractor::new(&sat_egraph, extraction_unser::MultDepth);
        let (_,best_node) = extractor.find_best(sat_egraph.find(concat_node));
        let explanation = sat_egraph.explain_equivalence(&start_expr, &best_node); 
        println!("{}", explanation.get_string());
        let extract_time = Instant::now() - start_time - sat_time;
    
        (extraction_unser::recexpr_traversal(best_node, &self.out_net_to_eclass), FlowStats {
            final_eclasses: sat_egraph.number_of_classes(),
            final_enodes: sat_egraph.total_number_of_nodes(),
            sat_time,
            extract_time
        })
    }

    fn md_vanilla_flow(mut self, initial_egraph: EGraph<Prop, ()>, concat_node: Id) -> (String,FlowStats) { 
        let start_time = Instant::now();
        // saturation
        let sat_egraph = self.saturate(initial_egraph, None);
        let sat_time = Instant::now() - start_time;
    
        // extraction
        let extractor = Extractor::new(&sat_egraph, extraction_unser::EsynDepth);
        let (_,best_node) = extractor.find_best(sat_egraph.find(concat_node));
        let extract_time = Instant::now() - start_time - sat_time;
    
        (extraction_unser::recexpr_traversal(best_node, &self.out_net_to_eclass), FlowStats {
            final_eclasses: sat_egraph.number_of_classes(),
            final_enodes: sat_egraph.total_number_of_nodes(),
            sat_time,
            extract_time
        })
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

    fn md_multiple_iters(mut self, initial_egraph: EGraph<Prop, ()>, iters: usize) -> (String,FlowStats) {
        let mut iter_initial_egraph = initial_egraph;
        let mut sat_time: Duration = Duration::from_secs(0);
        let mut extract_time: Duration = Duration::from_secs(0);
        let mut network: String = String::new();
        for i in 0..iters {
            //dbg!(&iter_initial_egraph);
            //iter_initial_egraph.dot().to_svg(format!("iter{}.svg", i)).unwrap();
            let start_time = Instant::now();
            // saturate
            let sat_egraph = self.saturate(iter_initial_egraph.clone(), Some(&iter_initial_egraph));
            let sat_time_iter = Instant::now() - start_time;
            sat_time += sat_time_iter;

            // convert to serialized graph
            // std::fs::File::create("out.egg").unwrap();
            let mut egg_file: std::fs::File = tempfile::tempfile().unwrap();
            serde::serialize_to_binfile(
                &sat_egraph,
                self.out_net_to_eclass.values().into_iter(),
                &mut egg_file, 
                |p| {
                    match p {
                        Prop::And(_) => 1.0,
                        _ => 0.0
                    }
                }).unwrap();
            // 2 egraphs in memory at same time is bad
            std::mem::drop(sat_egraph);
            egg_file.seek(std::io::SeekFrom::Start(0)).unwrap();
            let egraph_ser = egraph_serialize::EGraph::from_binary_file(&egg_file).unwrap();
            //for (k,_) in std::env::vars() {
            //    if k == "EGG_SERIALIZE" {
            //        egraph_ser.to_json_file("egraph.json").unwrap();
            //        break;
            //    }
            //}


            let mc_optimal = global_greedy_dag::mc_extract(&egraph_ser, &egraph_ser.root_eclasses);
            let mut mixedcost = extraction_ser::MixedCost {
                enode_opt_lookup: mc_optimal,
                results: IndexMap::new(),
                visited: HashSet::new()
            };
            for outnode_id in self.out_net_to_eclass.values() {
                mixedcost.select_best_eclass_mixed(&egraph_ser, egraph_serialize::ClassId::new(Into::<u32>::into(*outnode_id)), 0);
            }

            network = extraction_ser::dag_network_writer(&egraph_ser, &mut mixedcost.results, &self.out_net_to_eclass);
            extract_time += Instant::now() - start_time - sat_time_iter;

            // last iteration dont care about updating initial
            if i < iters-1 {
                deserialize_into_existing(&mut iter_initial_egraph, &mut self.new_to_old, egraph_ser, &mixedcost.results);
                // remap the output nodes back to this original graph
                for (_, old) in self.out_net_to_eclass.iter_mut() {
                    *old = *self.new_to_old.get(old).unwrap();
                }
            }            
        }
        (network, FlowStats {
            final_eclasses: 0,
            final_enodes: 0,
            sat_time,
            extract_time
        })
    }
}


//////////////////
// Main driver //
////////////////
struct FlowStats {
    final_eclasses: usize,
    final_enodes: usize,
    sat_time: Duration,
    extract_time: Duration
}

#[derive(PartialEq)]
enum FlowMode {
    McIlp,
    MdExplain,
    MdDag,
    MdMultipleIters,
    MdVanillaFlow
}

impl FromStr for FlowMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mc-ilp" => Ok(Self::McIlp),
            "md-explain" => Ok(Self::MdExplain),
            "md-dag" => Ok(Self::MdDag),
            "md-multiple-iters" => Ok(Self::MdMultipleIters),
            "md-vanilla-flow" => Ok(Self::MdVanillaFlow),
            _ => Err(()),
        }
    }
}

fn main() {
    env_logger::init();
    let mut args = std::env::args();
    args.next();
    let flow: FlowMode = args
        .next()
        .expect("No mode supplied!")
        .parse()
        .expect("Invalid mode!");

    args.next();
    let timeout_seconds = 20;//args.next().expect("No timeout given").parse::<u64>().expect("Invalid timeout").min(60);

    let start_expr_path = args.next().expect("No input expr file given!");
    let rules_path = args.next().expect("No input rules file given!");
    let output_eqn_path = args.next().expect("No output path given!");

    let rules_string = std::fs::read_to_string(rules_path).unwrap();
    let rules = process_rules(&rules_string);

    let start_string = std::fs::read_to_string(start_expr_path.clone()).unwrap();
    let mut start_lines = start_string.lines();
    let innodes = start_lines.next().unwrap();
    let outnodes = start_lines.next().unwrap();
    let start = start_lines.collect::<Vec<&str>>().join("\n");
    let (start_egraph, out_net_to_eclass, concat_node) = egraph_from_seqn(innodes, outnodes, start.as_str(), flow == FlowMode::MdExplain || flow == FlowMode::MdVanillaFlow);

    let opter = EqsatOptimizer::new(rules, out_net_to_eclass).with_timeout(timeout_seconds);

    let (network, stats) = match flow {
        FlowMode::McIlp => opter.mc_ilp_flow(start_egraph),
        FlowMode::MdExplain => opter.md_explain_flow(start_egraph, concat_node.unwrap()),
        FlowMode::MdDag => unimplemented!(), //opter.md_dag_flow(start_egraph),
        FlowMode::MdMultipleIters => opter.md_multiple_iters(start_egraph, 10),
        FlowMode::MdVanillaFlow => opter.md_vanilla_flow(start_egraph, concat_node.unwrap())
    };
    
    println!("{},{},{},{},{}", start_expr_path, stats.sat_time.as_secs(), stats.extract_time.as_secs(), stats.final_eclasses, stats.final_enodes);
    // output_eqn_path
    std::fs::write(output_eqn_path, format!(
        "INORDER = {};\nOUTORDER = {};\n{}",
        innodes, outnodes, network
    )).unwrap();
}
