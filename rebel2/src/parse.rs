// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, WordKind};
use std::str::CharIndices;

pub trait Collector {
    fn string(&mut self, string: &str) -> Result<(), CoreError>;
    fn word(&mut self, kind: WordKind, word: &str);
    fn integer(&mut self, value: i32);
    fn begin_block(&mut self);
    fn end_block(&mut self) -> Result<(), CoreError>;
}

pub struct Parser<'a, C>
where
    C: Collector,
{
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

    fn parse_string(&mut self, pos: usize) -> Result<(), CoreError> {
        let start_pos = pos + 1; // Skip the opening quote
        for (pos, char) in self.cursor.by_ref() {
            if char == '"' {
                return self.collector.string(
                    self.input
                        .get(start_pos..pos)
                        .ok_or(CoreError::EndOfInput)?,
                );
            }
        }
        Err(CoreError::EndOfInput)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<bool, CoreError> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    self.collector.word(
                        WordKind::SetWord,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(CoreError::EndOfInput)?,
                    );
                    return Ok(false);
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    self.collector.word(
                        WordKind::Word,
                        self.input
                            .get(start_pos..pos)
                            .ok_or(CoreError::EndOfInput)?,
                    );
                    return Ok(c == ']');
                }
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        self.collector.word(
            WordKind::Word,
            self.input.get(start_pos..).ok_or(CoreError::EndOfInput)?,
        );
        Ok(false)
    }

    fn parse_number(&mut self, char: char) -> Result<bool, CoreError> {
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
                value = c.to_digit(10).ok_or(CoreError::InternalError)? as i32;
                has_digits = true;
            }
            _ => return Err(CoreError::UnexpectedChar(char)),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    let digit = c.to_digit(10).ok_or(CoreError::InternalError)? as i32;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(CoreError::IntegerOverflow)?;
                }
                ']' => {
                    end_of_block = true;
                    break;
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(CoreError::EndOfInput);
        }
        if is_negative {
            value = value.checked_neg().ok_or(CoreError::IntegerOverflow)?;
        }
        self.collector.integer(value);
        Ok(end_of_block)
    }

    pub fn parse(&mut self) -> Result<(), CoreError> {
        while let Some((pos, char)) = self.skip_whitespace() {
            match char {
                '[' => self.collector.begin_block(),
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
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        Ok(())
    }
}
