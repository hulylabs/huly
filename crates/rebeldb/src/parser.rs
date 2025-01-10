use crate::model::{Hash, Transaction, Value};
use blake3::hash;
use std::str::Chars;

pub struct Parser<'a> {
    input: Chars<'a>,
    current: Option<char>,
    position: usize,
    transaction: &'a mut Transaction,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, transaction: &'a mut Transaction) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self {
            input: chars,
            current,
            position: 0,
            transaction,
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

    fn store_string(&mut self, s: &str) -> Hash {
        let bytes = s.as_bytes();
        let hash = *blake3::hash(bytes).as_bytes();
        self.transaction.blobs.insert(hash, bytes.to_vec());
        hash
    }

    fn parse_string(&mut self) -> Result<Value, String> {
        let mut result = String::new();
        // Skip opening quote
        self.advance();

        while let Some(c) = self.current {
            match c {
                '"' => {
                    self.advance(); // Skip closing quote
                    let hash = self.store_string(&result);
                    return Ok(Value::String(hash));
                }
                _ => {
                    result.push(c);
                    self.advance();
                }
            }
        }

        Err("Unterminated string literal".to_string())
    }

    fn parse_word(&mut self) -> Result<Value, String> {
        let mut result = String::new();

        while let Some(c) = self.current {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                result.push(c);
                self.advance();
            } else if c == ':' {
                self.advance();
                let hash = self.store_string(&result);
                return Ok(Value::SetWord(hash));
            } else {
                break;
            }
        }

        let hash = self.store_string(&result);
        Ok(Value::GetWord(hash))
    }

    fn parse_number(&mut self) -> Result<Value, String> {
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
            Ok(num) => {
                let num = if is_negative { -num } else { num };
                Ok(Value::Int64(num))
            }
            Err(_) => Err("Invalid number format".to_string()),
        }
    }

    fn parse_block(&mut self) -> Result<Value, String> {
        // Skip opening bracket
        self.advance();
        let mut values = Vec::new();

        loop {
            self.skip_whitespace();

            match self.current {
                None => return Err("Unterminated block".to_string()),
                Some(']') => {
                    self.advance();
                    break;
                }
                Some(_) => {
                    let value = self.parse_value()?;
                    values.push(value);
                }
            }
        }

        Ok(Value::Block(values.into_boxed_slice()))
    }

    fn parse_context(&mut self) -> Result<Value, String> {
        // We assume we're after the 'context' word and about to see a block
        self.skip_whitespace();

        match self.current {
            Some('[') => {
                self.advance();
                let mut pairs = Vec::new();

                loop {
                    self.skip_whitespace();

                    match self.current {
                        None => return Err("Unterminated context block".to_string()),
                        Some(']') => {
                            self.advance();
                            break;
                        }
                        Some(_) => {
                            // Parse word
                            let word = match self.parse_word()? {
                                Value::GetWord(hash) | Value::SetWord(hash) => hash,
                                _ => return Err("Expected word in context".to_string()),
                            };

                            // Expect and skip ':'
                            self.skip_whitespace();
                            if self.current != Some(':') {
                                return Err("Expected ':' after word in context".to_string());
                            }
                            self.advance();

                            // Parse value
                            self.skip_whitespace();
                            let value = self.parse_value()?;

                            pairs.push((word, value));
                        }
                    }
                }

                Ok(Value::Context(pairs.into_boxed_slice()))
            }
            _ => Err("Expected '[' after context".to_string()),
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();

        match self.current {
            None => Err("Unexpected end of input".to_string()),
            Some(c) => match c {
                '[' => self.parse_block(),
                '"' => self.parse_string(),
                c if c.is_alphabetic() => {
                    // Special handling for 'context' keyword
                    let start_pos = self.position;
                    let word = self.parse_word()?;

                    if let Value::GetWord(hash) = word {
                        if let Some(word_bytes) = self.transaction.blobs.get(&hash) {
                            if word_bytes == b"context" {
                                return self.parse_context();
                            }
                        }
                    }

                    Ok(word)
                }
                c if c.is_digit(10) || c == '-' => self.parse_number(),
                _ => Err(format!("Unexpected character: {}", c)),
            },
        }
    }

    pub fn parse(&mut self) -> Result<Value, String> {
        let value = self.parse_value()?;
        self.skip_whitespace();

        if self.current.is_some() {
            Err("Unexpected content after value".to_string())
        } else {
            Ok(value)
        }
    }
}

pub fn rebel_parse(transaction: &mut Transaction, input: &str) -> Result<Value, String> {
    let mut parser = Parser::new(input, transaction);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let mut tx = Transaction::new();

        let result = rebel_parse(&mut tx, r#"[x: "hello world" numbers: [1 2 3]]"#).unwrap();

        // We can't directly compare Values because they contain hashes
        // Instead, let's verify the structure matches what we expect
        if let Value::Block(items) = result.clone() {
            assert_eq!(items.len(), 4);
            // Further structural checks could be added here
        } else {
            panic!("Expected Block value");
        }

        tx.set_root(result);
        println!("{:#?}", tx);
    }
}
