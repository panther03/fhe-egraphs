use std::{collections::HashMap, path::PathBuf};

use crate::parse::{xag_to_sexpr, Token, Xag};
use crate::parse;

// TODO nasty recursive method but probably not going to fill the e-graph this way for large circuits anyhow
fn expand_xag(xag: Xag, expr_dict: &HashMap<String, Xag>) -> Xag {
    let newxag = match *(xag.op) {
        parse::XagOp::And(n1, n2) => Xag{inv: xag.inv, op: Box::new(parse::XagOp::And(expand_xag(n1, expr_dict), expand_xag(n2, expr_dict)))},
        parse::XagOp::Xor(n1, n2) => Xag{inv: xag.inv, op: Box::new(parse::XagOp::Xor(expand_xag(n1, expr_dict), expand_xag(n2, expr_dict)))},
        parse::XagOp::Ident(s) => {
            match expr_dict.get(&s) {
                Some(xag) => {expand_xag(xag.clone(), expr_dict)}
                None => Xag{inv: xag.inv, op: Box::new(parse::XagOp::Ident(s))}
            }
        }
        _ => xag
    };
    newxag
}

fn parse_nodes<'a>(line: &'a str, nodes: &mut Vec<&'a str>) -> bool {
    // last line
    let semicolon = line.contains(";");
    let line = line.trim();
    // remove last char from the string, without using String
    let line = if semicolon {&line[..line.len()-1]} else {line};
    let mut outnodes_line: Vec<&str> = line.split(" ").collect();
    nodes.append(&mut outnodes_line);
    semicolon
}

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
                    let xag = parse::infix_to_xag(split.next().unwrap());
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
        let outnode_contents = parse::xag_to_sexpr(outnode_xag);
        contents.push(' ');
        contents.push_str(&outnode_contents);
    }
    contents.push(')');
    
    std::fs::write(outsexpr, contents).unwrap();
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