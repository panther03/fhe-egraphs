use std::{collections::HashMap, path::PathBuf};

use crate::parse::Token;
use crate::parse;

fn postfix_to_expanded(postfix: Vec<Token>, expr_dict: &HashMap<String, String>) -> String {
    let mut output_str = String::new();
    let mut op_cnt_stack: Vec<i32> = Vec::new();
    let mut op_cnt = -1;
    for token in postfix.iter().rev() {
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
                op_cnt -= 1;
                match expr_dict.get(ident) {
                    Some(e) => output_str.push_str(e),
                    None => output_str.push_str(ident),
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

pub fn convert_eqn(ineqn: PathBuf, outsexpr: PathBuf, outnode: &str) {
    // open inrules and convert it to a vector of lines
    let lines = std::fs::read_to_string(ineqn).unwrap();

    let mut state: bool = false;
    let mut expr_dict: HashMap<String, String> = HashMap::new();
    for line in lines.lines() {
        if !state {
            state = (!line.contains("INORDER") && !line.contains("OUTORDER")) && line.contains("=");
        }
        if state && line.contains("=") {
            let mut split = line.split("=");
            let lhs = String::from(split.next().unwrap().trim());
            let rhs = postfix_to_expanded(
                parse::infix_str_to_postfix(split.next().unwrap()),
                &expr_dict,
            );
            expr_dict.insert(lhs.clone(), rhs);
        }
    }
    // take the last output for the time being
    let contents = expr_dict.get(outnode).unwrap_or_else(|| {panic!("Could not find node {} in circuit!", outnode);});
    std::fs::write(outsexpr, contents).unwrap();
}


