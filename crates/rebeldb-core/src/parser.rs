// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// parser.rs:

use crate::value::{Memory, Value};
use std::str::CharIndices;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unexpected end of input")]
    UnexpectedEnd,
    #[error("Number too large")]
    NumberTooLarge,
    #[error(transparent)]
    ValueError(#[from] crate::value::ValueError),
}

struct Token {
    value: Value,
    last_in_block: bool,
}

impl Token {
    fn new(value: Value, last_in_block: bool) -> Self {
        Self {
            value,
            last_in_block,
        }
    }
}

pub struct ValueIterator<'a, M>
where
    M: Memory,
{
    input: &'a str,
    cursor: CharIndices<'a>,
    memory: &'a mut M,
}

impl<M> Iterator for ValueIterator<'_, M>
where
    M: Memory,
{
    type Item = Result<Value, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_value()
            .map(|result| result.map(|token| token.value))
    }
}

impl<'a, M> ValueIterator<'a, M>
where
    M: Memory,
{
    pub fn new(input: &'a str, memory: &'a mut M) -> Self {
        Self {
            cursor: input.char_indices(),
            input,
            memory,
        }
    }

    fn skip_whitespace(&mut self) -> Option<(usize, char)> {
        for (pos, char) in self.cursor.by_ref() {
            if !char.is_ascii_whitespace() {
                return Some((pos, char));
            }
        }
        None
    }

    fn parse_string(&mut self, pos: usize) -> Result<Token, ParseError> {
        let start_pos = pos + 1; // skip the opening quote
        for (pos, char) in self.cursor.by_ref() {
            if char == '"' {
                return Ok(Token::new(
                    Value::string(self.memory, &self.input[start_pos..pos])?,
                    // Value::string(&self.input[start_pos..pos], self.blobs),
                    false,
                ));
            }
        }

        Err(ParseError::UnexpectedEnd)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<Token, ParseError> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    return Ok(Token::new(
                        Value::set_word(self.memory, &self.input[start_pos..pos])?,
                        false,
                    ))
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    return Ok(Token::new(
                        Value::word(self.memory, &self.input[start_pos..pos])?,
                        c == ']',
                    ))
                }
                _ => return Err(ParseError::UnexpectedChar(char)),
            }
        }
        Err(ParseError::UnexpectedEnd)
    }

    fn parse_number(&mut self, char: char) -> Result<Token, ParseError> {
        let mut value: i64 = 0;
        let mut is_negative = false;
        let mut has_digits = false;
        let mut end_of_block = false;

        match char {
            '+' => {}
            '-' => {
                is_negative = true;
            }
            c if c.is_ascii_digit() => {
                value = c.to_digit(10).unwrap() as i64;
                has_digits = true;
            }
            _ => return Err(ParseError::UnexpectedChar(char)),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(c.to_digit(10).unwrap() as i64))
                        .ok_or(ParseError::NumberTooLarge)?;
                }
                ']' => {
                    end_of_block = true;
                    break;
                }
                _ => break,
            }
        }

        if !has_digits {
            return Err(ParseError::UnexpectedEnd);
        }

        if is_negative {
            value = -value
        }
        Ok(Token::new(Value::new_int(value)?, end_of_block))
    }

    fn parse_value(&mut self) -> Option<Result<Token, ParseError>> {
        match self.skip_whitespace() {
            None => None,
            Some((pos, char)) => match char {
                '[' => self.parse_block(),
                '"' => Some(self.parse_string(pos)),
                c if c.is_ascii_alphabetic() => Some(self.parse_word(pos)),
                c if c.is_ascii_digit() || c == '+' || c == '-' => Some(self.parse_number(c)),
                _ => Some(Err(ParseError::UnexpectedChar(char))),
            },
        }
    }

    fn parse_block(&mut self) -> Option<Result<Token, ParseError>> {
        let mut values = Vec::<Value>::new();
        loop {
            match self.parse_value() {
                Some(Ok(Token {
                    value,
                    last_in_block,
                })) => {
                    values.push(value);
                    if last_in_block {
                        break;
                    }
                }
                Some(Err(err)) => return Some(Err(err)),
                None => {
                    if values.is_empty() {
                        return None;
                    } else {
                        break;
                    }
                }
            }
        }

        Some(
            Value::block(self.memory, &values)
                .map_err(ParseError::ValueError)
                .map(|v| Token::new(v, false)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::OwnMemory;

    #[test]
    fn test_whitespace_1() {
        let input = "  \t\n  ";
        let mut process = OwnMemory::new(65536, 1024);
        let mut iter = ValueIterator::new(input, &mut process);

        let value = iter.next();
        assert!(value.is_none());
    }

    #[test]
    fn test_string_1() {
        let input = "\"hello\"  \n ";
        let mut mem = OwnMemory::new(65536, 1024);
        let block: Vec<_> = ValueIterator::new(input, &mut mem)
            .filter_map(Result::ok)
            .collect();

        assert_eq!(block.len(), 1);
        assert_eq!(Value::as_inline_string(&mut mem, block[0]), Some("hello"));
    }

    #[test]
    fn test_number_1() -> anyhow::Result<()> {
        let input = "42";
        let mut process = OwnMemory::new(65536, 1024);
        let mut iter = ValueIterator::new(input, &mut process);

        let value = iter.next().unwrap().unwrap();
        assert_eq!(value.as_int(), Some(42));

        let value = iter.next();
        assert!(value.is_none());

        Ok(())
    }
}
