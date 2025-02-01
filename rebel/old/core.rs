// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::Word;
use crate::parse::{Collector, WordKind};
use crate::stack::{Stack, StackIter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebelError {
    #[error("out of memory")]
    OutOfMemory,
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

    pub fn len<H>(&self, memory: &Memory<H, (), (), ()>) -> Option<usize>
    where
        H: AsRef<[Word]>,
    {
        memory
            .heap
            .peek::<1>(self.address)
            .map(|[len]| *len as usize)
    }

    pub fn items<'a, H>(self, memory: &'a Memory<H, (), (), ()>) -> Option<StackIter<'a>>
    where
        H: AsRef<[Word]>,
    {
        let items_address = self.address + 1;
        self.len(memory)
            .and_then(|len| memory.heap.iter(items_address, len))
    }
}

// M E M O R Y

pub struct Memory<H, S, O, E> {
    heap: Stack<H>,
    stack: Stack<S>,
    ops: Stack<O>,
    env: Stack<E>,
}

impl<H, S, O, E> Memory<H, S, O, E>
where
    H: AsMut<[Word]> + AsRef<[Word]>,
    S: AsMut<[Word]>,
    O: AsMut<[Word]>,
    E: AsMut<[Word]>,
{
    pub fn new(heap: H, stack: S, ops: O, env: E) -> Self {
        Memory {
            heap: Stack::new(heap),
            stack: Stack::new(stack),
            ops: Stack::new(ops),
            env: Stack::new(env),
        }
    }
}

impl<'a> Memory<&'a mut [Word], &'a mut [Word], &'a mut [Word], &'a mut [Word]> {
    const OPS_SIZE: usize = 128;
    const ENV_SIZE: usize = 128;

    pub fn from_slice(memory: &'a mut [Word], heap_size: usize) -> Result<Self, RebelError> {
        let total_size = memory.len();
        let stack_size = total_size
            .checked_sub(heap_size + Self::OPS_SIZE + Self::ENV_SIZE)
            .ok_or(RebelError::OutOfMemory)?;

        let (heap_slice, rest) = memory.split_at_mut(heap_size);
        let (stack_slice, rest) = rest.split_at_mut(stack_size);
        let (ops_slice, env_slice) = rest.split_at_mut(Self::OPS_SIZE);

        Ok(Memory {
            heap: Stack::new(heap_slice),
            stack: Stack::new(stack_slice),
            ops: Stack::new(ops_slice),
            env: Stack::new(env_slice),
        })
    }
}

// I N T E R P R E T E R

pub struct Interpreter<'a, H, S, O, E> {
    memory: &'a mut Memory<H, S, O, E>,
}

impl<'a, H, S, O, E> Collector for Interpreter<'a, H, S, O, E>
where
    H: AsMut<[Word]>,
    S: AsMut<[Word]>,
    O: AsMut<[Word]>,
    E: AsMut<[Word]>,
{
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
