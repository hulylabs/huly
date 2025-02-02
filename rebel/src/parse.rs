// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Collector, RebelError, WordKind};
use std::str::CharIndices;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("integer overflow")]
    IntegerOverflow,
    #[error("internal error")]
    InternalError,
    #[error("memory error")]
    MemoryError,
}

pub struct Parser<'a, C: Collector> {
    input: &'a str,
    cursor: CharIndices<'a>,
    collector: &'a mut C,
}

impl<'a, C> Parser<'a, C>
where
    C: Collector,
{
    pub fn new(input: &'a str, collector: &'a mut C) -> Self {
        Self {
            input,
            collector,
            cursor: input.char_indices(),
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

    fn parse_string(&mut self, pos: usize) -> Result<(), RebelError> {
        let start_pos = pos + 1; // Skip the opening quote
        for (pos, char) in self.cursor.by_ref() {
            if char == '"' {
                return self.collector.string(
                    self.input
                        .get(start_pos..pos)
                        .ok_or(ParseError::EndOfInput)?,
                );
            }
        }
        Err(ParseError::EndOfInput.into())
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<bool, RebelError> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    self.collector.word(
                        WordKind::SetWord,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(ParseError::EndOfInput)?,
                    )?;
                    return Ok(false);
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    self.collector.word(
                        WordKind::Word,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(ParseError::EndOfInput)?,
                    )?;
                    return Ok(c == ']');
                }
                _ => return Err(ParseError::UnexpectedChar(char).into()),
            }
        }
        self.collector.word(
            WordKind::Word,
            self.input.get(start_pos..).ok_or(ParseError::EndOfInput)?,
        )?;
        Ok(false)
    }

    fn parse_number(&mut self, char: char) -> Result<bool, RebelError> {
        let mut value: i32 = 0;
        let mut is_negative = false;
        let mut has_digits = false;
        let mut end_of_block = false;

        match char {
            '+' => {}
            '-' => {
                is_negative = true;
            }
            c if c.is_ascii_digit() => {
                value = c.to_digit(10).ok_or(ParseError::InternalError)? as i32;
                has_digits = true;
            }
            _ => return Err(ParseError::UnexpectedChar(char).into()),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    let digit = c.to_digit(10).ok_or(ParseError::InternalError)? as i32;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(ParseError::IntegerOverflow)?;
                }
                ']' => {
                    end_of_block = true;
                    break;
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(ParseError::EndOfInput.into());
        }
        if is_negative {
            value = value.checked_neg().ok_or(ParseError::IntegerOverflow)?;
        }
        self.collector.integer(value).map(|_| end_of_block)
    }

    pub fn parse(&mut self) -> Result<(), RebelError> {
        while let Some((pos, char)) = self.skip_whitespace() {
            match char {
                '[' => self.collector.begin_block()?,
                ']' => self.collector.end_block()?,
                '"' => self.parse_string(pos)?,
                c if c.is_ascii_alphabetic() => {
                    if self.parse_word(pos)? {
                        self.collector.end_block()?;
                    }
                }
                c if c.is_ascii_digit() || c == '+' || c == '-' => {
                    if self.parse_number(c)? {
                        self.collector.end_block()?;
                    }
                }
                _ => return Err(ParseError::UnexpectedChar(char).into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{init_memory, EvalContext, Tag};
    use crate::Word;

    #[test]
    fn test_whitespace_1() -> Result<(), RebelError> {
        let input = "  \t\n  ";

        let mut buf = vec![0; 0x10000].into_boxed_slice();
        let mut mem = init_memory(&mut buf, 256, 1024)?;
        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new(input, &mut ctx);
        parser.parse()?;

        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), RebelError> {
        let input = "\"hello\"  \n ";

        let mut buf = vec![0; 0x10000].into_boxed_slice();
        let mut mem = init_memory(&mut buf, 256, 1024)?;
        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new(input, &mut ctx);
        parser.parse()?;

        Ok(())
    }

    #[test]
    fn test_block_1() -> Result<(), RebelError> {
        let input = "42 \"hello\" word x: \n ";

        let mut buf = vec![0; 0x10000].into_boxed_slice();
        let mut mem = init_memory(&mut buf, 256, 1024)?;
        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new(input, &mut ctx);
        parser.parse()?;

        let stack: Vec<_> = ctx.pop_parse().unwrap().collect();

        assert_eq!(stack.len(), 4);

        assert_eq!(stack[0][0], crate::core::TAG_INT);
        assert_eq!(stack[0][1], 42);

        // assert_eq!(stack[0], Tag::Int.into());
        // assert_eq!(stack[1], 42);
        // assert_eq!(stack[2], Tag::InlineString.into());
        // assert_eq!(stack[3], Tag::InlineString.into());
        // assert_eq!(stack[4], Tag::Word.into());

        // assert_eq!(stack[6], Tag::SetWord.into());

        // let v1 = block.get(&memory, 1).unwrap();
        // assert_eq!(memory.as_str(v1)?, "hello");
        Ok(())
    }

    // #[test]
    // fn test_number_1() -> Result<(), MemoryError> {
    //     let input = "42";

    //     let mut mem = vec![0; 0x10000];
    //     let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;

    //     let mut iter = ParseIterator::new(input, &mut layout);

    //     let value = iter.next().unwrap()?;
    //     assert_eq!(42 as i32, value.try_into()?);

    //     let value = iter.next();
    //     assert!(value.is_none());

    //     Ok(())
    // }
}
