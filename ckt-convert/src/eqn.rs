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

pub fn convert_eqn(ineqn: PathBuf, outsexpr: PathBuf, outnode: &str) {
    // open inrules and convert it to a vector of lines
    let lines = std::fs::read_to_string(ineqn).unwrap();

    let mut state: bool = false;
    let mut expr_dict: HashMap<String, Vec<Token>> = HashMap::new();
    for line in lines.lines() {
        if !state {
            state = (!line.contains("INORDER") && !line.contains("OUTORDER")) && line.contains("=");
        }
        if state && line.contains("=") {
            let mut split = line.split("=");
            let lhs = String::from(split.next().unwrap().trim());
            let postfix = parse::infix_str_to_postfix(split.next().unwrap());
            expr_dict.insert(lhs, postfix);
        }
    }
    // take the last output for the time being
    let mut top_postfix = expr_dict.remove(outnode).unwrap_or_else(|| {panic!("Could not find node {} in circuit!", outnode);});
    let contents = expand_postfix(
        &mut top_postfix,
        &expr_dict,
    );
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