// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use std::str::CharIndices;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError<E> {
    #[error("end of input")]
    EndOfInput,
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("integer overflow")]
    IntegerOverflow,
    #[error("unexpected error")]
    UnexpectedError,
    #[error("collector error: `{0}`")]
    CollectorError(E),
}

#[derive(Debug)]
pub enum WordKind {
    Word,
    SetWord,
}

pub trait Collector {
    type Error;

    fn string(&mut self, string: &str) -> Result<(), Self::Error>;
    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error>;
    fn integer(&mut self, value: i32) -> Result<(), Self::Error>;
    fn begin_block(&mut self) -> Result<(), Self::Error>;
    fn end_block(&mut self) -> Result<(), Self::Error>;
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

    pub fn parse_block(&mut self) -> Result<(), ParserError<C::Error>> {
        self.collector
            .begin_block()
            .map_err(ParserError::CollectorError)?;
        self.parse()?;
        self.collector
            .end_block()
            .map_err(ParserError::CollectorError)
    }

    fn skip_whitespace(&mut self) -> Option<(usize, char)> {
        for (pos, char) in self.cursor.by_ref() {
            if !char.is_ascii_whitespace() {
                return Some((pos, char));
            }
        }
        None
    }

    fn parse_string(&mut self, pos: usize) -> Result<(), ParserError<C::Error>> {
        let start_pos = pos + 1; // Skip the opening quote
        for (pos, char) in self.cursor.by_ref() {
            if char == '"' {
                return self
                    .collector
                    .string(
                        self.input
                            .get(start_pos..pos)
                            .ok_or(ParserError::EndOfInput)?,
                    )
                    .map_err(ParserError::CollectorError);
            }
        }
        Err(ParserError::EndOfInput)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<bool, ParserError<C::Error>> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    self.collector
                        .word(
                            WordKind::SetWord,
                            self.input
                                .get(start_pos..pos)
                                .ok_or(ParserError::EndOfInput)?,
                        )
                        .map_err(ParserError::CollectorError)?;
                    return Ok(false);
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    self.collector
                        .word(
                            WordKind::Word,
                            self.input
                                .get(start_pos..pos)
                                .ok_or(ParserError::EndOfInput)?,
                        )
                        .map_err(ParserError::CollectorError)?;
                    return Ok(c == ']');
                }
                _ => return Err(ParserError::UnexpectedChar(char)),
            }
        }
        self.collector
            .word(
                WordKind::Word,
                self.input.get(start_pos..).ok_or(ParserError::EndOfInput)?,
            )
            .map_err(ParserError::CollectorError)?;
        Ok(false)
    }

    fn parse_number(&mut self, char: char) -> Result<bool, ParserError<C::Error>> {
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
                value = c.to_digit(10).ok_or(ParserError::UnexpectedError)? as i32;
                has_digits = true;
            }
            _ => return Err(ParserError::UnexpectedChar(char)),
        }

        for (_, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_digit() => {
                    has_digits = true;
                    let digit = c.to_digit(10).ok_or(ParserError::UnexpectedError)? as i32;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(ParserError::IntegerOverflow)?;
                }
                ']' => {
                    end_of_block = true;
                    break;
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(ParserError::EndOfInput);
        }
        if is_negative {
            value = value.checked_neg().ok_or(ParserError::IntegerOverflow)?;
        }
        self.collector
            .integer(value)
            .map(|_| end_of_block)
            .map_err(ParserError::CollectorError)
    }

    fn parse(&mut self) -> Result<(), ParserError<C::Error>> {
        while let Some((pos, char)) = self.skip_whitespace() {
            match char {
                '[' => self
                    .collector
                    .begin_block()
                    .map_err(ParserError::CollectorError)?,
                ']' => self
                    .collector
                    .end_block()
                    .map_err(ParserError::CollectorError)?,
                '"' => self.parse_string(pos)?,
                c if c.is_ascii_alphabetic() => {
                    if self.parse_word(pos)? {
                        self.collector
                            .end_block()
                            .map_err(ParserError::CollectorError)?;
                    }
                }
                c if c.is_ascii_digit() || c == '+' || c == '-' => {
                    if self.parse_number(c)? {
                        self.collector
                            .end_block()
                            .map_err(ParserError::CollectorError)?;
                    }
                }
                _ => return Err(ParserError::UnexpectedChar(char)),
            }
        }
        Ok(())
    }
}
