use std::{collections::HashMap, path::PathBuf};

use crate::parse;
use crate::parse::{xag_to_sexpr, Token, Xag, XagOp};

pub struct Eqn<'a> {
    pub innodes: Vec<&'a str>,
    pub outnodes: Vec<&'a str>,
    pub lhses: Vec<String>,
    pub equations: HashMap<String, Xag>,
}

impl Eqn<'_> {
    fn new() -> Self {
        Self {
            innodes: Vec::new(),
            outnodes: Vec::new(),
            lhses: Vec::new(),
            equations: HashMap::new(),
        }
    }
}

/////////////////
// Eqn -> Xag //
///////////////

fn optimize_trivial_xor(xag: Xag) -> Xag {
    let newxag = match xag.op.as_ref() {
        parse::XagOp::And(n1, n2) => {
            let (ln1, ln2) = match n1.op.as_ref() {
                parse::XagOp::And(ln1, ln2) => (Some(ln1), Some(ln2)),
                _ => (None, None),
            };
            let (rn1, rn2) = match n2.op.as_ref() {
                parse::XagOp::And(rn1, rn2) => (Some(rn1), Some(rn2)),
                _ => (None, None),
            };
            match (ln1, ln2, rn1, rn2) {
                (Some(ln1), Some(ln2), Some(rn1), Some(rn2)) => {
                    if n1.inv
                        && n2.inv
                        && ln1.inv != rn1.inv
                        && ln2.inv != rn2.inv
                        && ln1.op == rn1.op
                        && ln2.op == rn2.op
                    {
                        Xag {
                            inv: !xag.inv,
                            op: Box::new(parse::XagOp::Xor(ln1.clone(), rn2.clone())),
                        }
                    } else {
                        xag
                    }
                }
                _ => xag,
            }
        }
        _ => xag,
    };
    newxag
}

fn expand_xag(xag: Xag, expr_dict: &HashMap<String, Xag>) -> Xag {
    let newxag = match *(xag.op) {
        parse::XagOp::And(n1, n2) => Xag {
            inv: xag.inv,
            op: Box::new(parse::XagOp::And(
                expand_xag(n1, expr_dict),
                expand_xag(n2, expr_dict),
            )),
        },
        parse::XagOp::Xor(n1, n2) => Xag {
            inv: xag.inv,
            op: Box::new(parse::XagOp::Xor(
                expand_xag(n1, expr_dict),
                expand_xag(n2, expr_dict),
            )),
        },
        parse::XagOp::Ident(s) => match expr_dict.get(&s) {
            Some(xag) => expand_xag(xag.clone(), expr_dict),
            None => Xag {
                inv: xag.inv,
                op: Box::new(parse::XagOp::Ident(s)),
            },
        },
        _ => xag,
    };
    newxag
}

fn parse_nodes<'a>(line: &'a str, nodes: &mut Vec<&'a str>) -> bool {
    // last line
    let semicolon = line.contains(";");
    let line = line.trim();
    // remove last char from the string, without using String
    let line = if semicolon {
        &line[..line.len() - 1]
    } else {
        line
    };
    let mut outnodes_line: Vec<&str> = line.split(" ").collect();
    nodes.append(&mut outnodes_line);
    semicolon
}

pub fn parse_eqn<'a>(ineqn: &'a String) -> Eqn<'a> {
    enum ParseState {
        Init,
        InOrder,
        OutOrder,
        Equations
    }   
    let mut state = ParseState::Init;
    let mut eqn = Eqn::new();
    // TODO: this can be much simplified, if we just split by semicolon instead of splitting by line break
    for line in ineqn.lines() {
        if line.is_empty() { continue; }
        state = match state {
            ParseState::Init => {
                // assume INORDER comes before OUTORDER
                if line.contains("INORDER") {
                    parse_nodes(line.split("=").nth(1).unwrap(), &mut eqn.innodes);
                    ParseState::InOrder
                } else {
                    ParseState::Init
                }
            },
            ParseState::InOrder => {
                if line.contains("OUTORDER") {
                    let semicolon = parse_nodes(line.split("=").nth(1).unwrap(), &mut eqn.outnodes);
                    if semicolon { ParseState::Equations } else { ParseState::OutOrder }
                } else {
                    parse_nodes(line, &mut eqn.innodes);
                    ParseState::InOrder
                }
            } 
            ParseState::OutOrder => {
                if parse_nodes(line, &mut eqn.outnodes) {
                    ParseState::Equations
                } else {
                    ParseState::OutOrder
                }
            }
            ParseState::Equations => {
                if line.contains("=") {
                    // surely no one would put inorder after outorder...
                    let mut split = line.split("=");
                    let lhs = String::from(split.next().unwrap().trim());
                    let xag = optimize_trivial_xor(parse::infix_to_xag(split.next().unwrap()));
                    eqn.lhses.push(lhs.clone());
                    eqn.equations.insert(lhs, xag);
                }
                ParseState::Equations
            }
        };
    }
    eqn
}

pub fn eqn2sexpr(ineqn: PathBuf, outsexpr: PathBuf, outnode: Option<&str>) {
    // open inrules and convert it to a vector of lines
    let lines = std::fs::read_to_string(ineqn).unwrap();
    let mut eqn = parse_eqn(&lines);
    let outnodes = match outnode {
        Some(node) => vec![node],
        None => eqn.outnodes
    };
    let mut contents = eqn.innodes.join(" ");
    contents.push('\n');
    contents.push_str(&outnodes.join(" "));
    contents.push_str("\n($");
    for outnode in outnodes {
        // take the last output for the time being
        let outnode_xag = eqn.equations.remove(outnode).unwrap_or_else(|| {panic!("Could not find node {} in circuit!", outnode);});
        let outnode_xag = expand_xag(
            outnode_xag,
            &eqn.equations,
        );
        let outnode_contents = parse::xag_to_sexpr(outnode_xag, false);
        contents.push(' ');
        contents.push_str(&outnode_contents);
    }
    contents.push(')');
    
    std::fs::write(outsexpr, contents).unwrap();
}

fn string_leaf(x: &XagOp) -> String {
    match &x {
        &XagOp::Lit(i) => i.to_string(),
        &XagOp::Ident(s) => s.clone(),
        _ => panic!()
    }
}

pub fn expr_to_list(lhs: String, x: &Xag, seen: &mut HashMap<String, ()>) -> Result<String,()> {
    let is_xor: bool = match x.op.as_ref() {
        &parse::XagOp::And(_,_) => false,
        &parse::XagOp::Xor(_,_) => true,
        _ => false// junk
    };
    
    let mut eqns = String::new();
    match x.op.as_ref() {
        // this is a disaster
        XagOp::And(n1, n2) | XagOp::Xor(n1, n2) => {
            let s1 = string_leaf(n1.op.as_ref());
            let s1_n = format!("{}_n", s1);
            let s1 = if n1.inv { 
                if seen.get(&s1_n).is_none() {
                    eqns.push_str(format!("{}=!;{};\n", s1_n, s1).as_str());
                    seen.insert(s1_n.clone(), ());
                }
                &s1_n
            } else { &s1 };
            let s2 = string_leaf(n2.op.as_ref());
            let s2_n = format!("{}_n", s2);
            let s2 = if n2.inv { 
                if seen.get(&s2_n).is_none() {
                    eqns.push_str(format!("{}=!;{};\n", s2_n, s2).as_str());
                    seen.insert(s2_n.clone(), ());
                }
                &s2_n
            } else { &s2 };
            let lhs_n = if x.inv { format!("{}_n", &lhs)} else { lhs.clone() };
            let mut thing= format!("{}{}={};{};{}\n", eqns, &lhs_n, if is_xor {"^"} else {"*"}, s1, s2);
            if x.inv {
                if seen.get(&lhs_n).is_none() {
                    seen.insert(lhs_n.clone(), ());
                }
                thing.push_str(format!("{}=!;{};\n", lhs, lhs_n).as_str());
            }
            Ok(thing)
        },
        XagOp::Lit(b) => Ok(format!("{}=w;{};\n", lhs, b)),
        XagOp::Ident(s) => {
            if x.inv {  
                Ok(format!("{}=!;{};\n", lhs, s))
            } else {
                Ok(format!("{}=w;{};\n", lhs, s))
            }
        },
        XagOp::Concat(_) => Err(())
    }
}

pub fn eqn2seqn(ineqn: PathBuf, outseqn: PathBuf) {
    let lines = std::fs::read_to_string(ineqn).unwrap();
    let mut eqn = parse_eqn(&lines);
    let mut contents = eqn.innodes.join(" ");
    contents.push('\n');
    contents.push_str(&(eqn.outnodes).join(" "));
    contents.push('\n');
    let mut seen: HashMap<String, ()> = HashMap::new();

    for lhs in eqn.lhses {
        let rhs = eqn.equations.get(&lhs).unwrap();
        let rhs = expr_to_list(lhs, rhs, &mut seen).unwrap();
        contents.push_str(rhs.as_str());
    }

    std::fs::write(outseqn, contents).unwrap();
}

fn xag2egglog(x: &Xag, outstr: &mut String, seen: &HashMap<String, ()>)  {
    if x.inv {
        outstr.push_str("(Not ");
    }
    match x.op.as_ref() {
        XagOp::Concat(ns) => unimplemented!(),
        XagOp::Xor(n1, n2) => {
            outstr.push_str("(Sum (multiset-of ");
            xag2egglog(n1, outstr, seen);
            outstr.push(' ');
            xag2egglog(n2, outstr, seen);
            outstr.push_str("))");
        },
        XagOp::And(n1, n2) => {
            outstr.push_str("(Product (multiset-of ");
            xag2egglog(n1, outstr, seen);
            outstr.push(' ');
            xag2egglog(n2, outstr, seen);
            outstr.push_str("))");
        },
        XagOp::Ident(i) => { 
            if seen.get(i).is_some() {
                outstr.push_str(i.as_str());
            } else {
                outstr.push_str(format!("(Var \"{}\")", i).as_str());
            }
        },
        _ => unimplemented!(),
    }
    if x.inv {
        outstr.push(')');
    }
}

pub fn eqn2egglog(ineqn: PathBuf, outseqn: PathBuf) {
    let lines = std::fs::read_to_string(ineqn).unwrap();
    let mut eqn = parse_eqn(&lines);

    let mut contents = String::new();
    let mut seen: HashMap<String, ()> = HashMap::new();

    for lhs in eqn.lhses {
        let rhs = eqn.equations.get(&lhs).unwrap();
        let mut line = format!("(let {} ", lhs);        
        xag2egglog(rhs, &mut line, &seen);
        line.push_str(")\n");
        contents.push_str(&line);
        seen.insert(lhs, ());
    }

    std::fs::write(outseqn, contents).unwrap();
}


/////////////////
// Xag -> Eqn //
///////////////

fn is_xag_leaf(xag: &Xag) -> Option<String> {
    match xag.op.as_ref() {
        XagOp::Lit(b) => Some((*b).to_string()),
        XagOp::Ident(s) => Some(s.clone()),
        _ => None,
    }
}

fn fresh_node(cnt: &mut u32) -> String {
    *cnt += 1;
    format!("n{}", *cnt-1)
}

type NodeLookup = (String,bool,String,bool,bool);

fn xag_outnode(xag: Xag, nodecnt: &mut u32, dedup: &mut HashMap<NodeLookup,u32>, eqnout: &mut String) -> String {
    match is_xag_leaf(&xag) {
        Some(s) => String::from(s),
        None => {
            let returned_cnt = xag_to_eqn(xag, *nodecnt, dedup, eqnout);
            // we should always either make progress or refer to an existing part of the tree
            assert!(returned_cnt != *nodecnt);
            format!("n{}", if returned_cnt > *nodecnt { *nodecnt = returned_cnt; returned_cnt-1 } else { returned_cnt })
        }
    }
}


fn xag_to_eqn(xag: Xag, nodecnt: u32, dedup: &mut HashMap<NodeLookup,u32>, eqnout: &mut String) -> u32 {
    let xagop = *xag.op;
    // only two operators possible
    let is_xor: bool = match &xagop {
        &parse::XagOp::And(_,_) => false,
        &parse::XagOp::Xor(_,_) => true,
        _ => false// junk
    };
    match xagop {
        parse::XagOp::And(n1, n2) | 
        parse::XagOp::Xor(n1, n2) => {
            let n1_inv = n1.inv;
            let n2_inv = n2.inv;

            let mut nodecnt = nodecnt; // including this node
            let n1_out = xag_outnode(n1, &mut nodecnt, dedup, eqnout);
            let n2_out = xag_outnode(n2, &mut nodecnt, dedup, eqnout);
            let nodes = (n1_out, n1_inv, n2_out, n2_inv, is_xor);

            match dedup.get(&nodes) {
                Some(n) => { *n },
                None => {
                    let outnode = fresh_node(&mut nodecnt);
                    eqnout.push_str(outnode.as_str());
                    eqnout.push_str(" = ");
                    if n1_inv {
                        eqnout.push('!');
                    }
                    eqnout.push_str(nodes.0.as_str());
                    if is_xor {
                        eqnout.push_str(" ^ ");
                    } else {
                        eqnout.push_str(" * ");
                    }
                    if n2_inv {
                        eqnout.push('!');
                    }
                    eqnout.push_str(nodes.2.as_str());
                    eqnout.push_str(";\n");
                    
                    dedup.insert(nodes, nodecnt - 1); // nodecnt - 1 = last node we added, which is this one
                    nodecnt
                }
            }
        },
        XagOp::Lit(b) => {
            let the_lit = if if xag.inv {b == 0} else {b != 0} { "true" } else { "false" };
            let mut nodecnt = nodecnt;
            let outnode = fresh_node(&mut nodecnt);
            eqnout.push_str(format!("{} = {};\n", outnode, the_lit).as_str());
            nodecnt
        },
        XagOp::Ident(s) => {
            let mut nodecnt = nodecnt;
            let outnode = fresh_node(&mut nodecnt);
            eqnout.push_str(&outnode);
            eqnout.push_str(" = ");
            if xag.inv { eqnout.push('!'); }
            eqnout.push_str(&s);
            eqnout.push_str(";\n");
            nodecnt
        },
        _ => unreachable!()
         // unless the graph has just one literal
    }
}


pub fn sexpr2eqn(insexpr: PathBuf, outeqn: PathBuf) {
    // open inrules and convert it to a vector of lines
    let sexpr = std::fs::read_to_string(insexpr).unwrap();
    let mut sexpr_lines = sexpr.lines();
    let innodes = sexpr_lines.next().unwrap();
    let outnodes = sexpr_lines.next().unwrap();
    let mut eqn = format!("INORDER = {};\nOUTORDER = {};\n", innodes, outnodes);
    let mut nodecnt = 0;
    let outnodes = outnodes.split(" ");
    let sexpr = parse::lex(sexpr_lines.next().unwrap());
    let mut dedup = HashMap::new();
    let xag = parse::sexpr_to_xag(sexpr);
    if let XagOp::Concat(xs) = *xag.op {
        let _ = xs
        .into_iter()
        .zip(outnodes)
        .for_each(|(x, on)| {
            let x_inv = x.inv;
            let new_nodecnt = xag_to_eqn(x, nodecnt, &mut dedup, &mut eqn);
            nodecnt = if new_nodecnt > nodecnt {new_nodecnt} else {nodecnt};
            if x_inv {
                eqn.push_str(format!("{} = !n{};\n", on, nodecnt-1).as_str());
            } else {
                eqn.push_str(format!("{} = n{};\n", on, nodecnt-1).as_str());
            }
        });
    }
    std::fs::write(outeqn, eqn).unwrap();
}

/*
mod tests {
    use crate::parse::infix_str_to_postfix;
    use super::parse::Token::*;
    use super::postfix_to_expanded;

    #[test]
    fn test_01() {
        let inp_string = "(n1010 * !n1009) + (!n1010 * n1009);";
        let postfix = infix_str_to_postfix(&inp_string);
        let expanded = postfix_to_expanded(postfix, expr_dict)
        assert!(postfix == vec![
            Ident(
                String::from("i25"),
            ),
            Not,
            Ident(
                String::from("i24"),
            ),
            Not,
            And,
            Ident(
                String::from("i26"),
            ),
            Not,
            And,
            Ident(
                String::from("i27"),
            ),
            Not,
            And,
        ])
    }
}


*/