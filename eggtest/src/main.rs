use egg::{*, rewrite as rw};

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

fn process_rules(rules_string: &str )-> Vec<Rewrite<Prop,()>> {
    let mut rules: Vec<Rewrite<Prop,()>> = Vec::new();
    let mut cnt = 0;
    for line in rules_string.lines() {
        let mut split = line.split(";");
        let lhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        let rhs: Pattern<Prop> = split.next().unwrap().parse().unwrap();
        cnt += 1;
        rules.push(rw!({cnt.to_string()}; {lhs} => {rhs}));
    }
    rules
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

fn main() {
    let start_expr_path  = std::env::args().nth(1).expect("No input expr file given!");
    let rules_path = std::env::args().nth(2).expect("No input rules file given!");

    let rules_string = std::fs::read_to_string(rules_path).unwrap();
    let rules = process_rules(&rules_string);

    let start_string = std::fs::read_to_string(start_expr_path).unwrap();
    let mut start_lines = start_string.lines();
    start_lines.next();
    start_lines.next();
    let start = String::from(start_lines.next().unwrap());
    let start = start.parse().unwrap();

    // That's it! We can run equality saturation now.
    let runner = Runner::default().with_expr(&start).run(rules.iter());

    // Extractors can take a user-defined cost function,
    // we'll use the egg-provided AstSize for now
    let extractor = Extractor::new(&runner.egraph, MultComplexity);

    // We want to extract the best expression represented in the
    // same e-class as our initial expression, not from the whole e-graph.
    // Luckily the runner stores the eclass Id where we put the initial expression.
    let (best_cost, best_expr) = extractor.find_best(runner.roots[0]);

    let runner: Runner<Prop, ()> = Runner::default().with_expr(&start);

    // Extractors can take a user-defined cost function,
    // we'll use the egg-provided AstSize for now
    let extractor = Extractor::new(&runner.egraph, MultComplexity);
    let (default_cost, _) = extractor.find_best(runner.roots[0]);


    println!("Before: {}", start);
    println!("After: {}", best_expr);
    println!("Cost : {} => {}", default_cost, best_cost);

    // we found the best thing, which is just "a" in this case
    //assert_eq!(best_expr, "a".parse().unwrap());
    //assert_eq!(best_cost, 1);

}
