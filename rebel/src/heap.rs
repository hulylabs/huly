// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

/// Memory layout:
/// 0 - bump_pointer
/// 1 - total_chunks
/// 2 - heap_start
/// 3 - symbol_table
/// 4 - opstack
/// 5 - stack
/// 6.. - heap

pub struct Memory<T> {
    data: T,
}

impl<T> Memory<T>
where
    T: AsMut<[u32]> + AsRef<[u32]>,
{
    const BUMP_POINTER: usize = 0;
    const TOTAL_CHUNKS: usize = 1;
    const HEAP_START: usize = 2;
    const SYMBOL_TABLE: usize = 3;

    const HEADER_SIZE: usize = 6;

    pub fn new(data: T) -> Option<Self> {
        let mut mem = Self { data };
        let heap_size = (mem.data.as_ref().len() as u32).checked_sub(Self::HEADER_SIZE as u32)?;
        mem.data
            .as_mut()
            .get_mut(..Self::HEADER_SIZE)
            .map(|header| {
                header[Self::BUMP_POINTER] = 0;
                header[Self::TOTAL_CHUNKS] = heap_size / 32;
                header[Self::HEAP_START] = Self::HEADER_SIZE as u32;
            })?;
        let symbol_table = mem.alloc(128)?;
        let opstack = mem.alloc(128)?;
        let stack = mem.alloc(128)?;
        mem.data
            .as_mut()
            .get_mut(Self::SYMBOL_TABLE..)?
            .copy_from_slice(&[symbol_table, opstack, stack]);
        Some(mem)
    }

    fn alloc(&mut self, chunks: u32) -> Option<u32> {
        if chunks == 0 {
            return None;
        }

        let header = self.data.as_mut().get_mut(..Self::HEADER_SIZE)?;

        let (bump_ptr, rest_header) = header.split_first_mut()?;
        let bump_pointer = *bump_ptr;

        let total_chunks = *rest_header.get(Self::TOTAL_CHUNKS)?;
        let heap_start = *rest_header.get(Self::HEAP_START)?;

        if bump_pointer + chunks <= total_chunks {
            *bump_ptr += chunks;
            Some(heap_start + bump_pointer * 32)
        } else {
            None
        }
    }
}

pub fn alloc(data: &mut [u32]) -> Option<()> {
    Memory::new(data)?;
    Some(())
}
