// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

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

pub struct ValueIterator<'a, 'b> {
    input: &'a str,
    cursor: CharIndices<'a>,
    memory: &'a mut Memory<'b>,
}

impl<'a, 'b> Iterator for ValueIterator<'a, 'b> {
    type Item = Result<Value, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parse_value()
            .map(|result| result.map(|token| token.value))
    }
}

impl<'a, 'b> ValueIterator<'a, 'b> {
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

    fn parse_string(&mut self, start_pos: usize) -> Result<Token, ParseError> {
        let mut pos = start_pos + 1;
        let bytes = self.input.as_bytes();

        // Find the end of string and validate without borrowing memory
        while pos < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(pos) };
            if b == b'"' {
                // Validate the found string
                if start_pos + 1 > pos || pos > bytes.len() {
                    return Err(ParseError::UnexpectedEnd);
                }

                // Only create string in memory once we have valid bounds
                let slice = unsafe { self.input.get_unchecked(start_pos + 1..pos) };
                let value = self.memory.new_string(slice)?;
                return Ok(Token::new(value, false));
            }

            // Handle UTF-8 sequences without borrowing
            if b & 0x80 == 0 {
                pos += 1;
            } else {
                let len = if b & 0xE0 == 0xC0 {
                    2
                } else if b & 0xF0 == 0xE0 {
                    3
                } else if b & 0xF8 == 0xF0 {
                    4
                } else {
                    return Err(ParseError::UnexpectedChar('\0'));
                };

                if pos + len > bytes.len() {
                    return Err(ParseError::UnexpectedEnd);
                }
                pos += len;
            }
        }

        Err(ParseError::UnexpectedEnd)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<Token, ParseError> {
        let mut pos = start_pos;
        let bytes = self.input.as_bytes();

        while pos < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(pos) };
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' => {
                    pos += 1;
                }
                b':' => {
                    let slice = unsafe { self.input.get_unchecked(start_pos..pos) };
                    let value = self.memory.set_word(slice)?;
                    return Ok(Token::new(value, false));
                }
                b' ' | b'\t' | b'\n' | b'\r' | b']' => {
                    let slice = unsafe { self.input.get_unchecked(start_pos..pos) };
                    let value = self.memory.word(slice)?;
                    return Ok(Token::new(value, b == b']'));
                }
                _ => return Err(ParseError::UnexpectedChar(b as char)),
            }
        }

        Err(ParseError::UnexpectedEnd)
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
        Ok(Token::new(Value::new_int(value), end_of_block))
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

pub fn parse(input: &str, memory: &mut Memory) -> Result<Vec<Value>, ParseError> {
    ValueIterator::new(input, memory)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_1() -> Result<(), ParseError> {
        let input = "  \t\n  ";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;
        let mut iter = ValueIterator::new(input, &mut layout);

        let value = iter.next();
        assert!(value.is_none());
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), ParseError> {
        let input = "\"hello\"  \n ";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;
        let block: Vec<_> = ValueIterator::new(input, &mut layout)
            .filter_map(Result::ok)
            .collect();

        assert_eq!(block.len(), 1);
        assert_eq!(layout.as_str(&block[0])?, "hello");
        Ok(())
    }

    #[test]
    fn test_number_1() -> Result<(), ParseError> {
        let input = "42";

        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;

        let mut iter = ValueIterator::new(input, &mut layout);

        let value = iter.next().unwrap().unwrap();
        assert_eq!(value.get_int(), Some(42));

        let value = iter.next();
        assert!(value.is_none());

        Ok(())
    }
}
