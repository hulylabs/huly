// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// parser.rs:

use crate::core::{Blobs, Block, Value};
use anyhow::{anyhow, Result};
use bytes::BytesMut;

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    blobs: &'a mut Blobs,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8], blobs: &'a mut Blobs) -> Self {
        Self {
            input,
            position: 0,
            blobs,
        }
    }

    fn current(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.current() {
            if !b.is_ascii_whitespace() {
                break;
            }
            self.advance();
        }
    }

    fn parse_string(&mut self) -> Result<Value> {
        // Skip opening quote
        self.advance();
        let mut buf = BytesMut::new();

        while let Some(b) = self.current() {
            match b {
                b'"' => {
                    self.advance(); // Skip closing quote
                    let hash = self.blobs.store(&buf);
                    return Ok(Value::String(hash));
                }
                b'\\' => {
                    self.advance();
                    match self.current() {
                        Some(b'"') => buf.extend_from_slice(&[b'"']),
                        Some(b'\\') => buf.extend_from_slice(&[b'\\']),
                        Some(b'n') => buf.extend_from_slice(&[b'\n']),
                        Some(b'r') => buf.extend_from_slice(&[b'\r']),
                        Some(b't') => buf.extend_from_slice(&[b'\t']),
                        Some(c) => return Err(anyhow!("Invalid escape sequence: \\{}", c as char)),
                        None => return Err(anyhow!("Unexpected end of string after \\")),
                    }
                    self.advance();
                }
                _ => {
                    buf.extend_from_slice(&[b]);
                    self.advance();
                }
            }
        }

        Err(anyhow!("Unterminated string literal"))
    }

    fn parse_word(&mut self) -> Result<Value> {
        let start = self.position;

        while let Some(b) = self.current() {
            match b {
                b if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'/' => {
                    self.advance();
                }
                b':' => {
                    let word = &self.input[start..self.position];
                    self.advance();
                    let hash = self.blobs.store(word);
                    return Ok(Value::SetWord(hash));
                }
                _ => break,
            }
        }

        let word = &self.input[start..self.position];
        let hash = self.blobs.store(word);
        Ok(Value::GetWord(hash))
    }

    fn parse_number(&mut self) -> Result<Value> {
        let mut value: u64 = 0;
        let mut has_sign = false;
        let mut is_negative = false;

        // Handle sign
        match self.current() {
            Some(b'+') => {
                has_sign = true;
                self.advance();
            }
            Some(b'-') => {
                has_sign = true;
                is_negative = true;
                self.advance();
            }
            _ => {}
        }

        // Parse integer part
        let mut has_digits = false;
        while let Some(b) = self.current() {
            if b.is_ascii_digit() {
                has_digits = true;
                value = value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add((b - b'0') as u64))
                    .ok_or_else(|| anyhow!("Number too large"))?;
                self.advance();
            } else {
                break;
            }
        }

        if !has_digits {
            return Err(anyhow!("Expected digits in number"));
        }

        // Return appropriate integer type
        if has_sign {
            if is_negative {
                let neg_value =
                    i64::try_from(value).map_err(|_| anyhow!("Number too large for i64"))?;
                Ok(Value::Int64(-neg_value))
            } else {
                let pos_value =
                    i64::try_from(value).map_err(|_| anyhow!("Number too large for i64"))?;
                Ok(Value::Int64(pos_value))
            }
        } else {
            Ok(Value::Uint64(value))
        }
    }

    fn parse_block(&mut self) -> Result<Value> {
        // Skip opening bracket
        self.advance();
        let mut values = Vec::new();

        loop {
            self.skip_whitespace();

            match self.current() {
                None => return Err(anyhow!("Unterminated block")),
                Some(b']') => {
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

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_whitespace();

        match self.current() {
            None => Err(anyhow!("Unexpected end of input")),
            Some(b) => match b {
                b'[' => self.parse_block(),
                b'"' => self.parse_string(),
                b if b.is_ascii_alphabetic() => self.parse_word(),
                b if b.is_ascii_digit() || b == b'+' || b == b'-' => self.parse_number(),
                _ => Err(anyhow!("Unexpected byte: {}", b)),
            },
        }
    }
}

pub fn parse(input: &str) -> Result<Block> {
    let mut blobs = Blobs::new();
    let value = {
        let mut parser = Parser::new(input.as_bytes(), &mut blobs);
        parser.parse_value()?
    };
    Ok(Block::new(value, blobs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string() -> Result<()> {
        let block = parse(r#""hello world""#)?;

        if let Value::String(hash) = block.root() {
            assert_eq!(
                String::from_utf8_lossy(block.get_blob(hash).unwrap()),
                "hello world"
            );
        } else {
            panic!("Expected String value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_block() -> Result<()> {
        let block = parse(r#"[x: 1 y: "test"]"#)?;

        if let Value::Block(items) = block.root() {
            assert_eq!(items.len(), 4);
        } else {
            panic!("Expected Block value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_nested() -> Result<()> {
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
    fn test_parse_escaped_string() -> Result<()> {
        let block = parse(r#""hello \"world\"""#)?;

        if let Value::String(hash) = block.root() {
            assert_eq!(
                String::from_utf8_lossy(block.get_blob(hash).unwrap()),
                r#"hello "world""#
            );
        } else {
            panic!("Expected String value");
        }
        Ok(())
    }

    #[test]
    fn test_parse_numbers() -> Result<()> {
        // Uint64
        let block = parse("123")?;
        assert!(matches!(block.root(), Value::Uint64(123)));

        // Int64 positive
        let block = parse("+123")?;
        assert!(matches!(block.root(), Value::Int64(123)));

        // Int64 negative
        let block = parse("-123")?;
        assert!(matches!(block.root(), Value::Int64(-123)));

        Ok(())
    }

    #[test]
    fn test_parse_words() -> Result<()> {
        // GetWord
        let block = parse("hello")?;
        assert!(matches!(block.root(), Value::GetWord(_)));

        // SetWord
        let block = parse("hello:")?;
        assert!(matches!(block.root(), Value::SetWord(_)));

        Ok(())
    }

    #[test]
    fn test_number_errors() -> Result<()> {
        // Just a sign
        assert!(parse("+").is_err());
        assert!(parse("-").is_err());

        // Number too large for i64
        assert!(parse("-9223372036854775809").is_err());

        Ok(())
    }
}
