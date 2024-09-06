use std::{collections::HashMap, path::PathBuf};

use crate::parse::Token;
use crate::parse;

fn expand_postfix(postfix: &mut Vec<Token>, expr_dict: &HashMap<String, Vec<Token>>) -> String {
    let mut output_str = String::new();
    let mut op_cnt_stack: Vec<i32> = Vec::new();
    let mut op_cnt = -1;
    while !postfix.is_empty() {
        let token = postfix.pop().unwrap();
        match token {
            Token::And => {
                op_cnt_stack.push(op_cnt);
                op_cnt = 2;
                output_str.push_str("(*")
            }
            Token::Xor => {
                op_cnt_stack.push(op_cnt);
                op_cnt = 2;
                output_str.push_str("(^")
            }
            Token::Or => {
                op_cnt_stack.push(op_cnt);
                op_cnt = 2;
                output_str.push_str("(+")
            }
            Token::Not => {
                op_cnt_stack.push(op_cnt);
                op_cnt = 1;
                output_str.push_str("(!")
            }
            Token::Ident(ident) => {
                match expr_dict.get(&ident) {
                    Some(tokens) => {
                        for token in tokens {
                            postfix.push(token.clone());
                        }
                        continue;
                    }
                    None => { 
                        output_str.push_str(&ident);
                        op_cnt -= 1;
                    }
                }
                
            }
            _ => {}
        }
        while op_cnt == 0 {
            output_str.push_str(")");
            op_cnt = op_cnt_stack.pop().unwrap_or(-1) - 1;
        }
        if op_cnt > 0 {
            output_str.push(' ');
        }
    }
    output_str
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
    let mut expr_dict: HashMap<String, Vec<Token>> = HashMap::new();
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
                    let postfix = parse::infix_str_to_postfix(split.next().unwrap());
                    expr_dict.insert(lhs, postfix);
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
    contents.push_str("\n(;");
    for outnode in outnodes {
        // take the last output for the time being
        let mut outnode_postfix = expr_dict.remove(outnode).unwrap_or_else(|| {panic!("Could not find node {} in circuit!", outnode);});
        let outnode_contents = expand_postfix(
            &mut outnode_postfix,
            &expr_dict,
        );
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