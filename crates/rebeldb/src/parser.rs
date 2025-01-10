use crate::core::{Blobs, Block, Value};
use std::str::Chars;

struct Parser<'a> {
    input: Chars<'a>,
    current: Option<char>,
    blobs: &'a mut Blobs,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, blobs: &'a mut Blobs) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self {
            input: chars,
            current,
            blobs,
        }
    }

    fn advance(&mut self) {
        self.current = self.input.next();
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current {
            if !c.is_whitespace() {
                break;
            }
            self.advance();
        }
    }

    fn parse_string(&mut self) -> Result<Value, String> {
        let mut result = String::new();
        // Skip opening quote
        self.advance();

        while let Some(c) = self.current {
            match c {
                '"' => {
                    self.advance(); // Skip closing quote
                    return Ok(self.blobs.string(&result));
                }
                '\\' => {
                    self.advance();
                    match self.current {
                        Some('"') => result.push('"'),
                        Some('\\') => result.push('\\'),
                        Some('n') => result.push('\n'),
                        Some('r') => result.push('\r'),
                        Some('t') => result.push('\t'),
                        Some(c) => return Err(format!("Invalid escape sequence: \\{}", c)),
                        None => return Err("Unexpected end of string after \\".to_string()),
                    }
                    self.advance();
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
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '/' {
                result.push(c);
                self.advance();
            } else if c == ':' {
                self.advance();
                return Ok(self.blobs.set_word(&result));
            } else {
                break;
            }
        }

        Ok(self.blobs.get_word(&result))
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
                Ok(Value::int64(num))
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

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();

        match self.current {
            None => Err("Unexpected end of input".to_string()),
            Some(c) => match c {
                '[' => self.parse_block(),
                '"' => self.parse_string(),
                c if c.is_alphabetic() => self.parse_word(),
                c if c.is_digit(10) || c == '-' => self.parse_number(),
                _ => Err(format!("Unexpected character: {}", c)),
            },
        }
    }
}

pub fn parse(input: &str) -> Result<Block, String> {
    let mut blobs = Blobs::new();
    let value = {
        let mut parser = Parser::new(input, &mut blobs);
        parser.parse_value()?
    };
    Ok(Block::new(value, blobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;

    #[test]
    fn test_parse_string() -> Result<(), String> {
        let block = parse(r#""hello world""#)?;

        if let Value::String(hash) = block.root() {
            assert_eq!(
                str::from_utf8(block.get_blob(hash).unwrap()).unwrap(),
                "hello world"
            );
        } else {
            panic!("Expected String value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_block() -> Result<(), String> {
        let block = parse(r#"[x: 1 y: "test"]"#)?;

        if let Value::Block(items) = block.root() {
            assert_eq!(items.len(), 4);
        } else {
            panic!("Expected Block value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_nested() -> Result<(), String> {
        let block = parse(r#"[points: [x: 10 y: 20] color: "red"]"#)?;

        if let Value::Block(items) = block.root() {
            assert_eq!(items.len(), 4);
            if let Value::Block(nested) = &items[1] {
                assert_eq!(nested.len(), 4);
            } else {
                panic!("Expected nested Block value");
            }
        } else {
            panic!("Expected Block value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_escaped_string() -> Result<(), String> {
        let block = parse(r#""hello \"world\"""#)?;

        if let Value::String(hash) = block.root() {
            assert_eq!(
                str::from_utf8(block.get_blob(hash).unwrap()).unwrap(),
                r#"hello "world""#
            );
        } else {
            panic!("Expected String value");
        }
        Ok(())
    }
}
