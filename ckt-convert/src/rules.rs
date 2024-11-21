use std::collections::HashMap;
use std::path::PathBuf;

use crate::parse::{Token, Xag};
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
    let lines = std::fs::read_to_string(&inrules).expect(format!("cannot open rules file: {:#?}", &inrules).as_str());

    let mut rules: String = String::new();
    let mut rule: Rule = Rule {
        lhs: None,
        rhs: None,
    };
    let mut rulecnt = rulecnt;
    for line in lines.lines() {
        if line.starts_with("old bexp") {
            let expr_string = line.split(":").nth(1).unwrap();
            rule.lhs = Some(parse::infix_to_sexpr_xag(expr_string, true));
        } else if line.starts_with("new bexp") {
            let expr_string = line.split(":").nth(1).unwrap();
            rule.rhs = Some(parse::infix_to_sexpr_xag(expr_string, true));
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

fn constant_fold(xag: Xag, maxpicount: u32) -> Xag {
    let newxag = match xag.op.as_ref() {
        parse::XagOp::And(n1, n2) => {
            let n1_cf = constant_fold(n1.to_owned(), maxpicount);
            let n2_cf = constant_fold(n2.to_owned(), maxpicount);
            let mut n = match (n1_cf.op.as_ref(), n2_cf.op.as_ref()) {
                (_, parse::XagOp::Lit(b)) => {                    
                    if *b ^ n2_cf.inv { n1_cf } else { n2_cf }
                },
                (parse::XagOp::Lit(b), _) => {
                    if *b ^ n1_cf.inv { n2_cf } else { n1_cf }
                },
                _ => Xag {
                    inv: false,
                    op: Box::new(parse::XagOp::And(n1_cf,n2_cf))
                },
            };
            n.inv ^= xag.inv;
            n
        }
        parse::XagOp::Xor(n1, n2) => {
            let mut n1_cf = constant_fold(n1.to_owned(), maxpicount);
            let mut n2_cf = constant_fold(n2.to_owned(), maxpicount);
            let mut n = match (n1_cf.op.as_ref(), n2_cf.op.as_ref()) {
                (_, parse::XagOp::Lit(b)) => {                    
                    if *b ^ n2_cf.inv { 
                        n1_cf.inv = !n1_cf.inv;
                    }
                    n1_cf
                }
                (parse::XagOp::Lit(b), _) => {
                    if *b ^ n1_cf.inv { 
                        n2_cf.inv = !n2_cf.inv;
                    }
                    n2_cf
                }
                _ => Xag {
                    inv: false,
                    op: Box::new(parse::XagOp::Xor(n1_cf,n2_cf))
                },
            };
            n.inv ^= xag.inv;
            n
        }
        parse::XagOp::Ident(s) => {
            let pi_n = (s[2..]).parse::<u32>();
            if pi_n.is_ok() && pi_n.unwrap() < (6 - maxpicount) {
                let mut x = xag;
                x.op = Box::new(parse::XagOp::Lit(false));
                x
            } else {
              xag
            }
        }
        _ => xag,
    };
    newxag
}

pub fn convert_cut_rewriting_rules(lhses: PathBuf, rhses: PathBuf, outrules: PathBuf) {
    let rhses = std::fs::read_to_string(rhses).unwrap();
    let lhses = std::fs::read_to_string(lhses).unwrap();

    let mut tt_to_rhs: HashMap<u64, Xag> = HashMap::new();
    for rhs in rhses.lines() {
        let mut rhs_split = rhs.split('=');
        let tt = rhs_split.next().unwrap().parse::<u64>().unwrap();
        let rhs_xag = parse::lex(rhs_split.next().unwrap());
        let rhs_xag = parse::sexpr_to_xag(rhs_xag);
        tt_to_rhs.insert(tt, rhs_xag);
    }

    let mut lhs_tts: HashMap<u64, u64> = HashMap::new();

    let mut rules = String::new();
    for lhs in lhses.lines() {
        let mut lhs_split = lhs.split('=');
        let maxpicount = lhs_split.next().unwrap().parse::<u32>().unwrap();
        let lhs_xag = lhs_split.next().unwrap();
        let tt = lhs_split.next().unwrap().parse::<u64>().unwrap();
        if lhs_tts.contains_key(&tt) || lhs_xag.contains("TERMINATED") || lhs_xag.contains("?nX") {
            continue;
        } else {
            lhs_tts.insert(tt,tt);
        }
        let rhs_xag = tt_to_rhs.get(&tt).unwrap().clone();
        let rhs_xag_cf = constant_fold(rhs_xag, maxpicount);
        let rhs_xag_cf = parse::xag_to_sexpr(rhs_xag_cf, false);
        rules.push_str(format!("{};{}\n", lhs_xag, rhs_xag_cf).as_str());
    }
    std::fs::write(outrules, rules).unwrap();
}
