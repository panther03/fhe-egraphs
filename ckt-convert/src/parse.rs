#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Token {
    Not,
    And,
    Xor,
    Or,
    LParen,
    RParen,
    Lit(bool),
    Ident(String),
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

pub fn infix_str_to_postfix(source: &str) -> Vec<Token> {
    infix_to_postfix(lex(source))
}

mod tests {
    use super::infix_str_to_postfix;
    use super::Token::*;

    #[test]
    fn test_01() {
        let inp_string = "((((not i25) and (not i24)) and (not i26)) and (not i27))";
        let postfix = infix_str_to_postfix(&inp_string);
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