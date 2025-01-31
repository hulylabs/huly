// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;
use crate::mem::Stack;

// S Y M B O L

pub struct InlineString {
    buf: [u32; 8],
}

impl InlineString {
    fn new(string: &str) -> Option<Self> {
        let bytes = string.as_bytes();
        let len = bytes.len();
        if len < 32 {
            let mut buf = [0; 8];
            for i in 0..len {
                buf[i / 4] |= (bytes[i] as u32) << ((i % 4) * 8);
            }
            Some(InlineString { buf })
        } else {
            None
        }
    }

    pub fn hash(&self) -> u32 {
        hash_u32x8(&self.buf)
    }
}

// S Y M B O L   T A B L E

pub struct SymbolTable<T> {
    data: T,
}

impl<T> SymbolTable<T>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    pub fn get_or_insert_symbol(
        &mut self,
        str: InlineString,
        heap: &mut Stack<T>,
    ) -> Option<Offset> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(count, data)| {
                let table_len = data.len() as u32;
                let h = str.hash();
                let mut index = h.checked_rem(table_len)?;

                for _probe in 0..table_len {
                    let offset = data.get_mut(index as usize)?;
                    let stored_offset = *offset;

                    if stored_offset == 0 {
                        let address = heap.push(str.buf)?;
                        *offset = address;
                        *count = *count + 1;
                        return Some(address);
                    }

                    if str.buf == heap.peek(stored_offset)? {
                        return Some(stored_offset);
                    }
                    index = (index + 1).checked_rem(table_len)?;
                }
                None
            })
    }
}

// pub fn get_or_insert_symbol(
//     table: &mut SymbolTable<Box<[Word]>>,
//     str: InlineString,
//     heap: &mut Stack<Box<[Word]>>,
// ) -> Option<Offset> {
//     table.get_or_insert_symbol(str, heap)
// }
