use std::path::PathBuf;

use crate::parse::Token;
use crate::parse;

#[derive(Clone, Debug)]
struct Rule {
    lhs: Option<String>,
    rhs: Option<String>,
}

fn postfix_to_prefix_rule(postfix: Vec<Token>) -> String {
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
            Token::Lit(b) => {
                op_cnt -= 1;
                output_str.push_str(if *b { "true" } else { "false" });
            }
            Token::Ident(ident) => {
                op_cnt -= 1;
                output_str.push('?');
                output_str.push_str(ident);
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

pub fn convert_rules(inrules: PathBuf, outrules: PathBuf, rulecnt: i32) {
    // open inrules and convert it to a vector of lines
    let lines = std::fs::read_to_string(inrules).unwrap();

    let mut rules: String = String::new();
    let mut rule: Rule = Rule {
        lhs: None,
        rhs: None,
    };
    let mut rulecnt = rulecnt;
    for line in lines.lines() {
        if line.starts_with("old bexp") {
            let expr_string = line.split(":").nth(1).unwrap();
            rule.lhs = Some(postfix_to_prefix_rule(parse::infix_str_to_postfix(expr_string)));
        } else if line.starts_with("new bexp") {
            let expr_string = line.split(":").nth(1).unwrap();
            rule.rhs = Some(postfix_to_prefix_rule(parse::infix_str_to_postfix(expr_string)));
        }

        if rule.lhs.is_some() && rule.rhs.is_some()
        {    
            if rulecnt == 0 {
                break;
            }
            let rule_str = format!(
                "{};{}\n",
                rule.lhs.unwrap().as_str(),
                rule.rhs.unwrap().as_str()
            );
            rules.push_str(&rule_str);
            rule = Rule {
                lhs: None,
                rhs: None,
            };
            rulecnt -= 1;
        } 
    }

    // write the rules to outrules
    std::fs::write(outrules, rules).unwrap();
}
