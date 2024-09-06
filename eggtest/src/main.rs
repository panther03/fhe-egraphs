use egg::{*, rewrite as rw};

define_language! {
    enum Prop {
        Bool(bool),
        "*" = And([Id; 2]),
        "!" = Not(Id),
        "+" = Or([Id; 2]),
        "^" = Xor([Id; 2]),
        // used for having multiple outputs
        ";" = Concat(Vec<Id>),
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
    let extractor = Extractor::new(&runner.egraph, AstSize);

    // We want to extract the best expression represented in the
    // same e-class as our initial expression, not from the whole e-graph.
    // Luckily the runner stores the eclass Id where we put the initial expression.
    let (best_cost, best_expr) = extractor.find_best(runner.roots[0]);

    println!("Before: {}", start);
    println!("After: {}", best_expr);

    // we found the best thing, which is just "a" in this case
    //assert_eq!(best_expr, "a".parse().unwrap());
    //assert_eq!(best_cost, 1);

}
