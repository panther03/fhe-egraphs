use std::{collections::HashMap, path::PathBuf};

use crate::parse;
use crate::parse::{xag_to_sexpr, Token, Xag, XagOp};

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

// TODO nasty recursive method but probably not going to fill the e-graph this way for large circuits anyhow
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

fn is_xag_leaf(xag: &Xag) -> Option<&str> {
    match xag.op.as_ref() {
        XagOp::Lit(b) => Some(if *b { "true" } else { "false" }),
        XagOp::Ident(s) => Some(s),
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
                    if is_xor {
                        /*if n1_inv {
                            eqnout.push('!');
                        }
                        eqnout.push_str(nodes.0.as_str());
                        eqnout.push_str(" ^ ");
                        if n2_inv {
                            eqnout.push('!');
                        }
                        eqnout.push_str(nodes.2.as_str());
                        eqnout.push_str(";\n");*/
                        eqnout.push_str(
                            format!("({}{} * {}{}) + ({}{} * {}{});\n",
                            if n1_inv {'!'} else {' '},
                            nodes.0.as_str(),
                            if !n2_inv {'!'} else {' '},
                            nodes.2.as_str(),
                            if !n1_inv {'!'} else {' '},
                            nodes.0.as_str(),
                            if n2_inv {'!'} else {' '},
                            nodes.2.as_str()
                            ).as_str()
                        );
                    } else {
                        if n1_inv {
                            eqnout.push('!');
                        }
                        eqnout.push_str(nodes.0.as_str());
                        eqnout.push_str(" * ");
                        if n2_inv {
                            eqnout.push('!');
                        }
                        eqnout.push_str(nodes.2.as_str());
                        eqnout.push_str(";\n");
                    }
                    
                    dedup.insert(nodes, nodecnt - 1); // nodecnt - 1 = last node we added, which is this one
                    nodecnt
                }
            }
        },
        XagOp::Lit(b) => {
            let the_lit = if b ^ xag.inv { "true" } else { "false" };
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

// TODO: this can be much simplified, if we just split by semicolon instead of splitting by line break
pub fn convert_eqn(ineqn: PathBuf, outsexpr: PathBuf, outnode: Option<&str>) {
    // open inrules and convert it to a vector of lines
    let lines = std::fs::read_to_string(ineqn).unwrap();

    enum ParseState {
        Init,
        InOrder,
        OutOrder,
        Equations
    }   
    let mut state = ParseState::Init;
    let mut innodes: Vec<&str> = Vec::new();
    let mut outnodes: Vec<&str> = Vec::new();
    let mut expr_dict: HashMap<String, Xag> = HashMap::new();
    for line in lines.lines() {
        if line.is_empty() { continue; }
        state = match state {
            ParseState::Init => {
                // assume INORDER comes before OUTORDER
                if line.contains("INORDER") {
                    parse_nodes(line.split("=").nth(1).unwrap(), &mut innodes);
                    ParseState::InOrder
                } else {
                    ParseState::Init
                }
            },
            ParseState::InOrder => {
                if line.contains("OUTORDER") {
                    let semicolon = parse_nodes(line.split("=").nth(1).unwrap(), &mut outnodes);
                    if semicolon { ParseState::Equations } else { ParseState::OutOrder }
                } else {
                    parse_nodes(line, &mut innodes);
                    ParseState::InOrder
                }
            } 
            ParseState::OutOrder => {
                if parse_nodes(line, &mut outnodes) {
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
                    expr_dict.insert(lhs, xag);
                }
                ParseState::Equations
            }
        };
    }
    let outnodes = match outnode {
        Some(node) => vec![node],
        None => outnodes
    };
    let mut contents = innodes.join(" ");
    contents.push('\n');
    contents.push_str(&outnodes.join(" "));
    contents.push_str("\n($");
    for outnode in outnodes {
        // take the last output for the time being
        let outnode_xag = expr_dict.remove(outnode).unwrap_or_else(|| {panic!("Could not find node {} in circuit!", outnode);});
        let outnode_xag = expand_xag(
            outnode_xag,
            &expr_dict,
        );
        let outnode_contents = parse::xag_to_sexpr(outnode_xag, false);
        contents.push(' ');
        contents.push_str(&outnode_contents);
    }
    contents.push(')');
    
    std::fs::write(outsexpr, contents).unwrap();
}

pub fn convert_sexpr(insexpr: PathBuf, outeqn: PathBuf) {
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
