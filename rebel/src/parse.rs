// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Value, inline_string};
use crate::mem::{Stack, Word, Offset};
use crate::module::Module;
use std::str::CharIndices;

#[derive(Debug)]
pub enum WordKind {
    Word,
    SetWord,
}

pub trait Collector {
    fn string(&mut self, string: &str) -> Option<()>;
    fn word(&mut self, kind: WordKind, word: &str) -> Option<()>;
    fn integer(&mut self, value: i32) -> Option<()>;
    fn begin_block(&mut self) -> Option<()>;
    fn end_block(&mut self) -> Option<()>;
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
                return self
                    .collector
                    .string(
                        self.input
                            .get(start_pos..pos)
                            .ok_or(CoreError::EndOfInput)?,
                    )
                    .ok_or(CoreError::ParseCollectorError);
            }
        }
        Err(CoreError::EndOfInput)
    }

    fn parse_word(&mut self, start_pos: usize) -> Result<bool, CoreError> {
        for (pos, char) in self.cursor.by_ref() {
            match char {
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
                ':' => {
                    self.collector
                        .word(
                            WordKind::SetWord,
                            self.input
                                .get(start_pos..pos)
                                .ok_or(CoreError::EndOfInput)?,
                        )
                        .ok_or(CoreError::ParseCollectorError)?;
                    return Ok(false);
                }
                c if c.is_ascii_whitespace() || c == ']' => {
                    self.collector
                        .word(
                            WordKind::Word,
                            self.input
                                .get(start_pos..pos)
                                .ok_or(CoreError::EndOfInput)?,
                        )
                        .ok_or(CoreError::ParseCollectorError)?;
                    return Ok(c == ']');
                }
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        self.collector
            .word(
                WordKind::Word,
                self.input.get(start_pos..).ok_or(CoreError::EndOfInput)?,
            )
            .ok_or(CoreError::ParseCollectorError)?;
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
        self.collector
            .integer(value)
            .map(|_| end_of_block)
            .ok_or(CoreError::ParseCollectorError)
    }

    pub fn parse(&mut self) -> Result<(), CoreError> {
        while let Some((pos, char)) = self.skip_whitespace() {
            match char {
                '[' => self
                    .collector
                    .begin_block()
                    .ok_or(CoreError::ParseCollectorError)?,
                ']' => self
                    .collector
                    .end_block()
                    .ok_or(CoreError::ParseCollectorError)?,
                '"' => self.parse_string(pos)?,
                c if c.is_ascii_alphabetic() => {
                    if self.parse_word(pos)? {
                        self.collector
                            .end_block()
                            .ok_or(CoreError::ParseCollectorError)?;
                    }
                }
                c if c.is_ascii_digit() || c == '+' || c == '-' => {
                    if self.parse_number(c)? {
                        self.collector
                            .end_block()
                            .ok_or(CoreError::ParseCollectorError)?;
                    }
                }
                _ => return Err(CoreError::UnexpectedChar(char)),
            }
        }
        Ok(())
    }
}

// P A R S E  C O L L E C T O R

pub struct ParseCollector<'a, T, B> {
    module: &'a mut Module<T, B>,
    pub parse: Stack<[Word; 64]>,
    ops: Stack<[Word; 32]>,
}

impl<'a, T, B> ParseCollector<'a, T, B> {
    pub fn new(module: &'a mut Module<T, B>) -> Self {
        Self {
            module,
            parse: Stack::new([0; 64]),
            ops: Stack::new([0; 32]),
        }
    }
}

impl<T, B> Collector for ParseCollector<'_, T, B>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
    B: crate::module::BlobStore,
{
    fn string(&mut self, string: &str) -> Option<()> {
        let offset = self.module.get_heap_mut().alloc(inline_string(string)?)?;
        self.parse.push([Value::TAG_INLINE_STRING, offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Option<()> {
        let symbol = inline_string(word)?;
        let id = self.module.get_symbols_mut()?.get_or_insert(symbol)?;
        let tag = match kind {
            WordKind::Word => Value::TAG_WORD,
            WordKind::SetWord => Value::TAG_SET_WORD,
        };
        self.parse.push([tag, id])
    }

    fn integer(&mut self, value: i32) -> Option<()> {
        self.parse.push([Value::TAG_INT, value as u32])
    }

    fn begin_block(&mut self) -> Option<()> {
        self.ops.push([self.parse.len()?])
    }

    fn end_block(&mut self) -> Option<()> {
        let [bp] = self.ops.pop()?;
        let block_data = self.parse.pop_all(bp)?;
        let offset = self.module.get_heap_mut().alloc_block(block_data)?;
        self.parse.push([Value::TAG_BLOCK, offset])
    }
}

// Public parse function that can be used to parse a string into a block of code
pub fn parse<T, B>(module: &mut Module<T, B>, code: &str) -> Result<Offset, CoreError> 
where
    T: AsMut<[Word]> + AsRef<[Word]>,
    B: crate::module::BlobStore,
{
    let mut collector = ParseCollector::new(module);
    collector
        .begin_block()
        .ok_or(CoreError::ParseCollectorError)?;
    Parser::new(code, &mut collector).parse()?;
    collector
        .end_block()
        .ok_or(CoreError::ParseCollectorError)?;
    let result = collector.parse.pop::<2>().ok_or(CoreError::InternalError)?;
    Ok(result[1])
}
