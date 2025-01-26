// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::Word;
use crate::parse::{Collector, WordKind};
use crate::stack::{Stack, StackIter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebelError {
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("integer overflow")]
    IntegerOverflow,
    #[error("internal error")]
    InternalError,
}

// T A G

#[repr(u32)]
enum Tag {
    Int,
    Block,
}

impl From<Tag> for Word {
    fn from(tag: Tag) -> Self {
        tag as Word
    }
}

// B L O C K

pub struct Block {
    address: Word,
}

impl Block {
    fn new(address: Word) -> Self {
        Self { address }
    }

    pub fn len(self, memory: &Memory) -> Option<usize> {
        memory
            .heap
            .peek::<1>(self.address)
            .map(|[len]| *len as usize)
    }

    pub fn items<'a>(self, memory: &'a Memory<'a>) -> Option<StackIter<'a>> {
        let items_address = self.address + 1;
        self.len(memory)
            .and_then(|len| memory.heap.iter(items_address, len))
    }
}

// M E M O R Y

pub struct Memory<'a> {
    heap: Stack<'a>,
    stack: Stack<'a>,
    ops: Stack<'a>,
    env: Stack<'a>,
}

impl<'a> Memory<'a> {
    pub fn new(heap: Stack<'a>, stack: Stack<'a>, ops: Stack<'a>, env: Stack<'a>) -> Self {
        Self {
            heap,
            stack,
            ops,
            env,
        }
    }
}

// I N T E R P R E T E R

pub struct Interpreter<'a> {
    memory: &'a mut Memory<'a>,
}

impl<'a> Collector for Interpreter<'a> {
    fn string(&self, string: &str) -> Result<(), RebelError> {
        unimplemented!()
    }

    fn word(&self, kind: WordKind, word: &str) -> Result<(), RebelError> {
        unimplemented!()
    }

    fn integer(&mut self, value: i32) -> Result<(), RebelError> {
        self.memory
            .stack
            .push([Tag::Int.into(), value as Word])
            .ok_or(RebelError::StackOverflow)
    }

    fn begin_block(&self) -> Result<(), RebelError> {
        unimplemented!()
    }

    fn end_block(&self) -> Result<(), RebelError> {
        unimplemented!()
    }
}
