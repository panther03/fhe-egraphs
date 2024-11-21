#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Token {
    Not,
    And,
    Xor,
    Or,
    Concat,
    LParen,
    RParen,
    Lit(bool),
    Ident(String),
}

#[derive(Clone,Debug,PartialEq)]
pub struct Xag {
    pub inv: bool,
    pub op: Box<XagOp>
}

#[derive(Clone,Debug,PartialEq)]
pub enum XagOp {
    Concat(Vec<Xag>),
    Xor(Xag, Xag),
    And(Xag, Xag),
    Ident(String),
    Lit(bool)
}

impl Xag {
    pub fn get_name<'a> (&'a self) -> Option<&'a str> {
        match self.op.as_ref() {
            XagOp::Ident(s) => Some(s.as_str()),
            _ => None // idk
        }
    }
    pub fn leaves_iter(&mut self, leaf_handle: &dyn Fn(&mut String)) {
        match self.op.as_mut() {
            XagOp::Concat(ns) => {ns.iter_mut().for_each(|x: &mut Xag| x.leaves_iter(leaf_handle));},
            XagOp::Xor(n1, n2) => { n1.leaves_iter(leaf_handle); n2.leaves_iter(leaf_handle); },
            XagOp::And(n1, n2) => { n1.leaves_iter(leaf_handle); n2.leaves_iter(leaf_handle); },
            XagOp::Ident(i) => { leaf_handle(i); },
            _ => {},
        }
    }
}


pub fn lex(source: &str) -> Vec<Token> {
    let source_surround = format!("({})", source);

    let mut tokens = Vec::new();
    let mut ctr = -1;
    let mut flush: bool;
    let mut curr_token = String::new();
    let mut new_tokens: Vec<Token> = Vec::new();
    for char in source_surround.chars() {
        flush = true;
        match char {
            '(' => { new_tokens.push(Token::LParen) },
            ')' => { new_tokens.push(Token::RParen) },
            '!' => { ctr = 1; new_tokens.push(Token::LParen); new_tokens.push(Token::Not) },
            '*' => { new_tokens.push(Token::And) },
            '+' => { new_tokens.push(Token::Or) },
            '^' => { new_tokens.push(Token::Xor) },
            '$' => { new_tokens.push(Token::Concat)},
            ' '|';' => {},
            _ => { curr_token.push(char); flush = false;}
        }

        if flush && !curr_token.is_empty() {
            match curr_token.as_str() {
                "and" => tokens.push(Token::And),
                "or" => tokens.push(Token::Or),
                "xor" => tokens.push(Token::Xor),
                "not" => tokens.push(Token::Not),
                "0" | "false" => tokens.push(Token::Lit(false)),
                "1" | "true" => tokens.push(Token::Lit(true)),
                _ => tokens.push(Token::Ident(curr_token.clone())),
            }
            curr_token = String::new();
            ctr -= 1;
        }

        tokens.append(&mut new_tokens);
        if ctr == 0 {
            tokens.push(Token::RParen);
            ctr = -1;
        }
    }
    tokens
}

fn infix_to_postfix(infix: Vec<Token>) -> Vec<Token> {
    let mut stack: Vec<Token> = Vec::new();
    let mut postfix: Vec<Token> = Vec::new();

    for token in infix {
        match token {
            Token::Ident(_) | Token::Lit(_) => postfix.push(token),
            Token::LParen => stack.push(token),
            Token::RParen => {
                while *stack.last().unwrap() != Token::LParen {
                    let tok = stack.pop().unwrap();
                    postfix.push(tok);
                }
                stack.pop();
            }
            _ => stack.push(token),
        }
    }

    postfix
}

pub fn postfix_to_xag(postfix: &Vec<Token>) -> Xag {
    let mut nodes: Vec<Xag> = Vec::new();
    for token in postfix.iter() {
        let new_node = match token {
            Token::And => {
                let n1 = nodes.pop().unwrap();
                let n2 = nodes.pop().unwrap();
                Xag { inv: false, op: Box::new(XagOp::And(n1, n2)) }
            }
            Token::Xor => {
                let n1 = nodes.pop().unwrap();
                let n2 = nodes.pop().unwrap();
                Xag { inv: false, op: Box::new(XagOp::Xor(n1, n2)) }  
            }
            Token::Or => {
                // demorgans
                let mut n1 = nodes.pop().unwrap();
                n1.inv = !n1.inv;
                let mut n2 = nodes.pop().unwrap();
                n2.inv = !n2.inv;
                Xag { inv: true, op: Box::new(XagOp::And(n1, n2)) }  
            }
            Token::Concat => {
                let mut new_nodes: Vec<Xag> = Vec::new();
                new_nodes.append(&mut nodes);
                new_nodes.reverse();
                Xag { inv: false, op: Box::new(XagOp::Concat(new_nodes))}
            }
            Token::Not => {
                let mut n = nodes.pop().unwrap();
                n.inv = !n.inv;
                n
            }
            Token::Lit(b) => {
                Xag { inv: false, op: Box::new(XagOp::Lit(*b)) }
            }
            Token::Ident(ident) => {
                Xag { inv: false, op: Box::new(XagOp::Ident(ident.to_string())) }
            }
            _ => { panic!("Postfix should not have parentheses!"); }
        };
        nodes.push(new_node);
    }
    nodes.pop().unwrap()
}

pub fn xag_to_sexpr(xag: Xag, question_identifiers: bool) -> String {
    let mut output_str = String::new();
    let mut op_cnt_stack: Vec<i32> = Vec::new();
    let mut op_cnt = -1;
    let mut nodes: Vec<Xag> = vec![xag];
    while !nodes.is_empty() {
        let node = nodes.pop().unwrap();
        if node.inv {
            op_cnt_stack.push(op_cnt);
            op_cnt = 1;
            output_str.push_str("(! ");
        }
        match *node.op {
            XagOp::And(n1, n2) => {
                nodes.push(n1);
                nodes.push(n2);
                op_cnt_stack.push(op_cnt);
                op_cnt = 2;
                output_str.push_str("(*");
            }
            XagOp::Xor(n1, n2) => {
                nodes.push(n1);
                nodes.push(n2);
                op_cnt_stack.push(op_cnt);
                op_cnt = 2;
                output_str.push_str("(^");
            }
            XagOp::Concat(mut ns) => {
                let ns_len = ns.len();
                nodes.append(&mut ns);
                op_cnt_stack.push(op_cnt);
                op_cnt = ns_len as i32;
                output_str.push_str("($");
            }
            XagOp::Lit(b) => {
                op_cnt -= 1;
                output_str.push_str( if b {"true"} else {"false"});
            }
            XagOp::Ident(s) => {
                op_cnt -= 1;
                if question_identifiers {output_str.push('?');}
                output_str.push_str(&s);
            }
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

pub fn sexpr_to_xag(sexpr: Vec<Token>) -> Xag {
    // filter out lparen and rparen from sexpr while keeping the type the same
    let mut postfix: Vec<Token> = sexpr.into_iter().filter(|t| match t {
        Token::LParen | Token::RParen => false,
        _ => true,
    }).collect();
    postfix.reverse();
    let xag = postfix_to_xag(&postfix);
    xag
}


pub fn infix_to_xag(source: &str) -> Xag {
    postfix_to_xag(&infix_to_postfix(lex(source)))
}

pub fn infix_to_sexpr_xag(source: &str, question_identifiers: bool) -> String {
    xag_to_sexpr(postfix_to_xag(&infix_to_postfix(lex(source))), question_identifiers)
}

mod tests {
    use crate::parse::infix_to_sexpr_xag;

    use super::infix_to_xag;
    use super::Token::*;

    #[test]
    fn test_01() {
        let inp_string = "((((not i25) and (not i24)) xor (not i26)) or (not i27))";
        //let inp_string = "i24 and i24";
        let xag = infix_to_sexpr_xag(&inp_string, false);
        dbg!(xag);
        /*
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
        ])*/
    }
}
