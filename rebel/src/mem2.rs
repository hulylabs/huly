// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::hash::hash_u32x8;

pub type Word = u32;
pub type Offset = Word;
pub type Symbol = Offset;

trait StructMut<T>
where
    Self: Sized,
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize;
    const ITEM_SIZE: usize;

    fn init(data: T) -> Option<Self>;
}

struct Base<T>(T);

impl<T> Base<T>
where
    T: AsMut<[Word]>,
{
    fn init_header(mut self) -> Option<Self> {
        self.0.as_mut().first_mut().map(|hdr| *hdr = 0);
        Some(self)
    }

    fn alloc<'a, S>(&'a mut self, items: usize) -> Option<(Offset, S)>
    where
        S: StructMut<&'a mut [Word]>,
    {
        self.0.as_mut().split_first_mut().and_then(|(len, data)| {
            let start = *len as usize;
            let end = start + items * S::ITEM_SIZE + S::HDR_SIZE;
            data.get_mut(start..end)
                .and_then(|block| S::init(block))
                .map(|s| (start as Offset, s))
        })
    }
}

// H E A P

pub struct Heap<T>(Base<T>);

impl<T> StructMut<T> for Heap<T>
where
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize = 1;
    const ITEM_SIZE: usize = 1;

    fn init(data: T) -> Option<Self> {
        Base(data).init_header().map(Self)
    }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    fn alloc<'a, S>(&'a mut self, items: usize) -> Option<(Offset, S)>
    where
        S: StructMut<&'a mut [Word]>,
    {
        self.0.alloc(items)
    }
}

// S T A C K

pub struct Stack<T>(Base<T>);

impl<T> StructMut<T> for Stack<T>
where
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize = 1;
    const ITEM_SIZE: usize = 1;

    fn init(data: T) -> Option<Self> {
        Base(data).init_header().map(Self)
    }
}

//

pub fn test<'a>(heap: &'a mut Heap<[Word; 1024]>) -> Option<(Offset, Stack<&'a mut [Word]>)> {
    heap.alloc::<Stack<_>>(16)
}
