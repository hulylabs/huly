// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Address, Word};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// M E M O R Y

pub struct Memory<T> {
    data: T,
    heap: Address,
    stack: Address,
    ops: Address,
}

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self, address: Address) -> Option<usize> {
        self.data
            .as_ref()
            .get(address as usize)
            .map(|len| *len as usize)
    }

    fn slice_get(&self, address: Address) -> Option<&[Word]> {
        let address = address as usize;
        let len = self.data.as_ref().get(address).copied()? as usize;
        self.data.as_ref().get(address + 1..address + len)
    }

    fn get_heap(&self) -> Option<Stack<&[Word]>> {
        self.slice_get(self.heap).map(Stack::new)
    }
}

impl<T> Memory<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    const STACK_SIZE: u32 = 1024;
    const OPS_SIZE: u32 = 256;

    pub fn new(data: T, heap: Address) -> Option<Self> {
        let len = data.as_ref().len() as Address;
        let stack = len.checked_sub(Self::STACK_SIZE)?;
        let ops = stack.checked_sub(Self::OPS_SIZE)?;
        let heap_size = ops.checked_sub(heap)?;

        let mut mem = Self {
            data,
            heap,
            stack,
            ops,
        };

        mem.alloc(heap, heap_size)?;
        mem.alloc(stack, Self::STACK_SIZE)?;
        mem.alloc(ops, Self::OPS_SIZE)?;

        Some(mem)
    }

    fn slice_get_mut(&mut self, address: Address) -> Option<&mut [Word]> {
        let address = address as usize;
        let len = self.data.as_ref().get(address).copied()? as usize;
        self.data.as_mut().get_mut(address + 1..address + len)
    }

    fn alloc(&mut self, address: Address, size: Address) -> Option<()> {
        self.data
            .as_mut()
            .get_mut(address as usize)
            .map(|len| *len = size)
    }

    fn get_heap_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.heap).map(Stack::new)
    }

    fn get_stack_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.stack).map(Stack::new)
    }

    fn get_ops_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.ops).map(Stack::new)
    }
}

// S T A C K

pub struct Stack<T> {
    data: T,
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }

    fn peek<const N: usize>(&self, offset: Address) -> Option<[Word; N]> {
        let offset = offset as usize;
        self.data.as_ref().split_first().and_then(|(_, slot)| {
            slot.get(offset..offset + N)
                .and_then(|slot| slot.try_into().ok())
        })
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                let len = *size as usize;
                let remaining = slot.len() - len;
                if remaining < N {
                    None
                } else {
                    *size += N as u32;
                    slot.get_mut(len..len + N).map(|items| {
                        items
                            .iter_mut()
                            .zip(value.iter())
                            .for_each(|(slot, value)| {
                                *slot = *value;
                            })
                    })
                }
            })
    }

    fn pop<const N: usize>(&mut self) -> Option<[Word; N]> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                size.checked_sub(N as u32).and_then(|sp| {
                    let len = sp as usize;
                    slot.get(len..len + N).map(|slot| {
                        let mut value = [0; N];
                        value.iter_mut().zip(slot.iter()).for_each(|(value, slot)| {
                            *value = *slot;
                        });
                        *size = sp;
                        value
                    })
                })
            })
    }
}

// S Y M B O L

pub struct Symbol {
    buf: [u32; 8],
}

impl Symbol {
    fn new(string: &str) -> Option<Self> {
        let bytes = string.as_bytes();
        let len = bytes.len();
        if len < 32 {
            let mut buf = [0; 8];
            for i in 0..len {
                buf[i / 4] |= (bytes[i] as u32) << ((i % 4) * 8);
            }
            Some(Symbol { buf })
        } else {
            None
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn hash(&self) -> u32 {
        unsafe {
            const C1: u32 = 0xcc9e2d51;
            const C2: u32 = 0x1b873593;
            const R1: i32 = 15;
            const M: u32 = 5;
            const N: u32 = 0xe6546b64;

            // Load all constants into vectors once
            let vc1 = vdupq_n_u32(C1);
            let vc2 = vdupq_n_u32(C2);
            let vm = vdupq_n_u32(M);
            let vn = vdupq_n_u32(N);

            // Load both chunks at once
            let (chunk1, chunk2) = {
                let ptr = self.buf.as_ptr();
                (vld1q_u32(ptr), vld1q_u32(ptr.add(4)))
            };

            // Process both chunks
            let k1 = vmulq_u32(
                vmulq_u32(
                    vorrq_u32(vshlq_n_u32(chunk1, R1), vshrq_n_u32(chunk1, 32 - R1)),
                    vc2,
                ),
                vc1,
            );

            let k2 = vmulq_u32(
                vmulq_u32(
                    vorrq_u32(vshlq_n_u32(chunk2, R1), vshrq_n_u32(chunk2, 32 - R1)),
                    vc2,
                ),
                vc1,
            );

            // Combine results
            let h1 = vaddq_u32(vmulq_u32(veorq_u32(k1, k2), vm), vn);

            // Horizontal add to get final value
            let mut result = vgetq_lane_u32(h1, 0)
                ^ vgetq_lane_u32(h1, 1)
                ^ vgetq_lane_u32(h1, 2)
                ^ vgetq_lane_u32(h1, 3);

            // Final mixing (kept in scalar as it's sequential)
            result ^= result >> 16;
            result = result.wrapping_mul(0x85ebca6b);
            result ^= result >> 13;
            result = result.wrapping_mul(0xc2b2ae35);
            result ^= result >> 16;

            result
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    #[inline(always)]
    unsafe fn hash_avx2(&self) -> u32 {
        // AVX2 implementation
        #[allow(overflowing_literals)]
        let vc1 = _mm256_set1_epi32(0xcc9e2d51_u32 as i32);
        #[allow(overflowing_literals)]
        let vc2 = _mm256_set1_epi32(0x1b873593_u32 as i32);
        #[allow(overflowing_literals)]
        let vn = _mm256_set1_epi32(0xe6546b64_u32 as i32);

        let chunks = _mm256_loadu_si256(self.buf.as_ptr() as *const __m256i);

        let rotated = _mm256_or_si256(_mm256_slli_epi32(chunks, 15), _mm256_srli_epi32(chunks, 17));

        let h1 = _mm256_add_epi32(
            _mm256_mullo_epi32(
                _mm256_mullo_epi32(_mm256_mullo_epi32(rotated, vc2), vc1),
                _mm256_set1_epi32(5),
            ),
            vn,
        );

        let h2 = _mm256_xor_si256(h1, _mm256_permute4x64_epi64(h1, 0b10_11_00_01));
        let h3 = _mm256_xor_si256(h2, _mm256_shuffle_epi32(h2, 0b10_11_00_01));

        let mut result = _mm256_extract_epi32(h3, 0) as u32;

        result ^= result >> 16;
        result = result.wrapping_mul(0x85ebca6b);
        result ^= result >> 13;
        result = result.wrapping_mul(0xc2b2ae35);
        result ^= result >> 16;

        result
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    #[inline(always)]
    unsafe fn hash_sse41(&self) -> u32 {
        // SSE4.1 implementation (the optimized version we had)
        #[allow(overflowing_literals)]
        let vc1 = _mm_set1_epi32(0xcc9e2d51_u32 as i32);
        #[allow(overflowing_literals)]
        let vc2 = _mm_set1_epi32(0x1b873593_u32 as i32);
        #[allow(overflowing_literals)]
        let vn = _mm_set1_epi32(0xe6546b64_u32 as i32);

        let chunk1 = _mm_loadu_si128(self.buf.as_ptr() as *const __m128i);
        let chunk2 = _mm_loadu_si128(self.buf.as_ptr().add(4) as *const __m128i);

        let rotated1 = _mm_or_si128(_mm_slli_epi32(chunk1, 15), _mm_srli_epi32(chunk1, 17));

        let rotated2 = _mm_or_si128(_mm_slli_epi32(chunk2, 15), _mm_srli_epi32(chunk2, 17));

        let h1 = _mm_add_epi32(
            _mm_mullo_epi32(
                _mm_xor_si128(
                    _mm_mullo_epi32(_mm_mullo_epi32(rotated1, vc2), vc1),
                    _mm_mullo_epi32(_mm_mullo_epi32(rotated2, vc2), vc1),
                ),
                _mm_set1_epi32(5),
            ),
            vn,
        );

        let h2 = _mm_shuffle_epi32(h1, 0b10_11_00_01);
        let h3 = _mm_xor_si128(h1, h2);
        let h4 = _mm_shuffle_epi32(h3, 0b00_01_10_11);
        let mut result = _mm_cvtsi128_si32(_mm_xor_si128(h3, h4)) as u32;

        result ^= result >> 16;
        result = result.wrapping_mul(0x85ebca6b);
        result ^= result >> 13;
        result = result.wrapping_mul(0xc2b2ae35);
        result ^= result >> 16;

        result
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn hash(&self) -> u32 {
        unsafe {
            // Try AVX2 first
            if is_x86_feature_detected!("avx2") {
                self.hash_avx2()
            } else {
                self.hash_sse41()
            }
        }
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    pub fn hash(&self) -> u32 {
        const C1: u32 = 0xcc9e2d51;
        const C2: u32 = 0x1b873593;
        const R1: u32 = 15;
        const R2: u32 = 13;
        const M: u32 = 5;
        const N: u32 = 0xe6546b64;

        let mut h1: u32 = 0; // seed = 0

        for &k1 in self.buf.iter() {
            let mut k = k1.wrapping_mul(C1);
            k = k.rotate_left(R1);
            k = k.wrapping_mul(C2);

            h1 ^= k;
            h1 = h1.rotate_left(R2);
            h1 = h1.wrapping_mul(M).wrapping_add(N);
        }

        h1 ^= h1 >> 16;
        h1 = h1.wrapping_mul(0x85ebca6b);
        h1 ^= h1 >> 13;
        h1 = h1.wrapping_mul(0xc2b2ae35);
        h1 ^= h1 >> 16;

        h1
    }
}

pub fn new_symbol(string: &str) -> Option<Symbol> {
    Symbol::new(string)
}

pub fn hash(sym: &Symbol) -> u32 {
    sym.hash()
}

type MemoryBox = Memory<Box<[Word]>>;
type StackRef<'a> = Stack<&'a mut [Word]>;
type StackRefRO<'a> = Stack<&'a [Word]>;

pub fn push_1(x: &mut StackRef, v: Word) -> Option<()> {
    x.push([v])
}

pub fn push_3(x: &mut StackRef, v: Word, y: Word, z: Word) -> Option<()> {
    x.push([v, y, z])
}

pub fn pop_3(x: &mut StackRef) -> Option<[u32; 3]> {
    x.pop()
}

pub fn pop_1(x: &mut StackRef) -> Option<[u32; 1]> {
    x.pop()
}

pub fn move_3(from: &mut StackRef, to: &mut StackRef) -> Option<()> {
    from.pop::<3>().and_then(|value| to.push(value))
}

pub fn peek_1(x: &StackRef, offset: u32) -> Option<[u32; 1]> {
    x.peek(offset)
}

pub fn peek_3(x: &StackRef, offset: u32) -> Option<[u32; 3]> {
    x.peek(offset)
}

pub fn mem_push_3(mem: &mut MemoryBox, address: Address, v: Word, y: Word, z: Word) -> Option<()> {
    mem.slice_get_mut(address)
        .and_then(|stack| StackRef::new(stack).push([v, y, z]))
}

pub fn mem_pop_3(x: &mut MemoryBox, address: Address) -> Option<[u32; 3]> {
    x.slice_get_mut(address)
        .and_then(|stack| StackRef::new(stack).pop())
}

pub fn mem_peek_3(x: &MemoryBox, address: Address, offset: u32) -> Option<[u32; 3]> {
    x.slice_get(address)
        .and_then(|stack| StackRefRO::new(stack).peek(offset))
}

// impl<T> Stack<T>
// where
//     T: AsRef<[Word]>,
// {
//     pub fn peek<const N: usize>(&self, address: Address) -> Option<&[Word; N]> {
//         let address = address as usize;
//         if address + N > self.sp {
//             None
//         } else {
//             self.data
//                 .as_ref()
//                 .get(address..address + N)
//                 .and_then(|slot| slot.try_into().ok())
//         }
//     }

//     pub fn iter(&self, address: Address, len: usize) -> Option<StackIter> {
//         let address = address as usize;
//         if address + len > self.sp {
//             None
//         } else {
//             self.data
//                 .as_ref()
//                 .get(address..address + len)
//                 .map(|data| data.iter())
//         }
//     }

// O W N E D  S T A C K

// pub struct OwnedStack {
//     data: Box<[Word]>,
//     stack: Stack<'static>,
// }

// impl OwnedStack {
//     pub fn new(size: usize) -> Self {
//         let data = vec![0; size].into_boxed_slice();
//         Self {
//             data,
//             stack: Stack::new(&mut data.as_mut()),
//         }
//     }

//     pub fn into_stack(&self) -> &mut Stack<'_> {
//         Stack::new(&mut self.data)
//     }
// }
