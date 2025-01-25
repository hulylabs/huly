// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::value::{Block, Memory, Value};
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
    MemoryError(#[from] crate::value::MemoryError),
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

struct ParseIterator<'a, 'b> {
    input: &'a str,
    cursor: CharIndices<'a>,
    memory: &'a mut Memory<'b>,
}

impl Iterator for ParseIterator<'_, '_> {
    type Item = Result<Value, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_value()
            .map(|result| result.map(|token| token.value))
    }
}

impl<'a, 'b> ParseIterator<'a, 'b> {
    pub fn new(input: &'a str, memory: &'a mut Memory<'b>) -> Self {
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
                    self.memory.string(
                        self.input
                            .get(start_pos..pos)
                            .ok_or(ParseError::UnexpectedEnd)?,
                    )?,
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
                        self.memory.set_word(
                            self.input
                                .get(start_pos..pos)
                                .ok_or(ParseError::UnexpectedEnd)?,
                        )?,
                        false,
                    ))
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    return Ok(Token::new(
                        self.memory.word(
                            self.input
                                .get(start_pos..pos)
                                .ok_or(ParseError::UnexpectedEnd)?,
                        )?,
                        c == ']',
                    ))
                }
                _ => return Err(ParseError::UnexpectedChar(char)),
            }
        }
        Ok(Token::new(
            self.memory.word(
                self.input
                    .get(start_pos..)
                    .ok_or(ParseError::UnexpectedEnd)?,
            )?,
            false,
        ))
    }

    fn parse_number(&mut self, char: char) -> Result<Token, ParseError> {
        let mut value: u32 = 0;
        let mut is_negative = false;
        let mut has_digits = false;
        let mut end_of_block = false;

        match char {
            '+' => {}
            '-' => {
                is_negative = true;
            }
            c if c.is_ascii_digit() => {
                value = c.to_digit(10).unwrap();
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
                        .and_then(|v| v.checked_add(c.to_digit(10).unwrap()))
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

        let value: i32 = if is_negative {
            -(value as i32)
        } else {
            value as i32
        };

        Ok(Token::new(Value::from(value), end_of_block))
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
        let stack_start = self.memory.stack_pointer();
        loop {
            match self.parse_value() {
                Some(Ok(Token {
                    value,
                    last_in_block,
                })) => {
                    match self.memory.push(value) {
                        Ok(_) => {}
                        Err(err) => return Some(Err(err.into())),
                    }
                    if last_in_block {
                        break;
                    }
                }
                Some(Err(err)) => return Some(Err(err)),
                None => {
                    if self.memory.stack_pointer() == stack_start {
                        return None;
                    } else {
                        break;
                    }
                }
            }
        }

        Some(
            self.memory
                .block(stack_start)
                .map_err(ParseError::MemoryError)
                .map(|v| Token::new(v, false)),
        )
    }
}

pub fn parse_block(memory: &mut Memory, input: &str) -> Result<Block, ParseError> {
    let mut iterator = ParseIterator::new(input, memory);
    if let Some(token) = iterator.parse_block() {
        let block = token?.value.try_into()?;
        Ok(block)
    } else {
        Err(ParseError::UnexpectedEnd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_1() -> Result<(), ParseError> {
        let input = "  \t\n  ";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;
        let mut iter = ParseIterator::new(input, &mut layout);

        let value = iter.next();
        assert!(value.is_none());
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), ParseError> {
        let input = "\"hello\"  \n ";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;
        let block: Vec<_> = ParseIterator::new(input, &mut layout)
            .filter_map(Result::ok)
            .collect();

        assert_eq!(block.len(), 1);
        assert_eq!(layout.as_str(block[0])?, "hello");
        Ok(())
    }

    #[test]
    fn test_block_1() -> Result<(), ParseError> {
        let input = "42 \"hello\" word x: \n ";

        let mut bytes = vec![0; 0x10000];
        let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;
        let block = parse_block(&mut memory, input)?;

        assert_eq!(block.len(&memory), Some(4));

        let v1 = block.get(&memory, 1).unwrap();
        assert_eq!(memory.as_str(v1)?, "hello");
        Ok(())
    }

    #[test]
    fn test_number_1() -> Result<(), ParseError> {
        let input = "42";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;

        let mut iter = ParseIterator::new(input, &mut layout);

        let value = iter.next().unwrap()?;
        assert_eq!(42 as i32, value.try_into()?);

        let value = iter.next();
        assert!(value.is_none());

        Ok(())
    }
}
