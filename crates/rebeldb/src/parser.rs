//

use crate::model::Transaction;
use std::str::Chars;

#[derive(Debug, PartialEq)]
pub enum Token {
    LBracket,        // [
    RBracket,        // ]
    Integer(i64),    // 123, -456
    Word(String),    // x, point, numbers
    SetWord(String), // x:
    String(String),  // "hello world"
    Error(String),   // For error reporting
}

pub struct Tokenizer<'a> {
    input: Chars<'a>,
    current: Option<char>,
    position: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self {
            input: chars,
            current,
            position: 0,
        }
    }

    fn advance(&mut self) {
        self.current = self.input.next();
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current {
            if !c.is_whitespace() {
                break;
            }
            self.advance();
        }
    }

    fn read_string(&mut self) -> Token {
        let mut result = String::new();
        // Skip opening quote
        self.advance();

        while let Some(c) = self.current {
            match c {
                '"' => {
                    self.advance(); // Skip closing quote
                    return Token::String(result);
                }
                _ => {
                    result.push(c);
                    self.advance();
                }
            }
        }

        Token::Error("Unterminated string literal".to_string())
    }

    fn read_word(&mut self) -> Token {
        let mut result = String::new();

        while let Some(c) = self.current {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                result.push(c);
                self.advance();
            } else if c == ':' {
                self.advance();
                return Token::SetWord(result);
            } else {
                break;
            }
        }

        Token::Word(result)
    }

    fn read_number(&mut self) -> Token {
        let mut result = String::new();
        let mut is_negative = false;

        // Handle negative numbers
        if self.current == Some('-') {
            is_negative = true;
            self.advance();
        }

        while let Some(c) = self.current {
            if c.is_digit(10) {
                result.push(c);
                self.advance();
            } else {
                break;
            }
        }

        match result.parse::<i64>() {
            Ok(num) => Token::Integer(if is_negative { -num } else { num }),
            Err(_) => Token::Error("Invalid number format".to_string()),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();

        let token = match self.current {
            None => return None,
            Some(c) => match c {
                '[' => {
                    self.advance();
                    Token::LBracket
                }
                ']' => {
                    self.advance();
                    Token::RBracket
                }
                '"' => self.read_string(),
                c if c.is_alphabetic() => self.read_word(),
                c if c.is_digit(10) || c == '-' => self.read_number(),
                _ => {
                    self.advance();
                    Token::Error(format!("Unexpected character: {}", c))
                }
            },
        };

        Some(token)
    }
}

// Parser implementation will go here
pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
    transaction: &'a mut Transaction,
    current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, transaction: &'a mut Transaction) -> Self {
        let mut tokenizer = Tokenizer::new(input);
        let current_token = tokenizer.next();
        Self {
            tokenizer,
            transaction,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.tokenizer.next();
    }

    // More parser methods will be added here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer() {
        let input = r#"[x: "hello world" numbers: [1 2 3]]"#;
        let tokens: Vec<Token> = Tokenizer::new(input).collect();

        assert_eq!(
            tokens,
            vec![
                Token::LBracket,
                Token::SetWord("x".to_string()),
                Token::String("hello world".to_string()),
                Token::SetWord("numbers".to_string()),
                Token::LBracket,
                Token::Integer(1),
                Token::Integer(2),
                Token::Integer(3),
                Token::RBracket,
                Token::RBracket,
            ]
        );
    }
}
