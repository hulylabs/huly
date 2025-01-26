// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Address, Word};

// S T A C K

pub type StackIter<'a> = std::slice::Iter<'a, Word>;

pub struct Stack<'a> {
    data: &'a mut [Word],
    sp: usize,
}

impl<'a> Stack<'a> {
    pub fn new(data: &'a mut [Word]) -> Self {
        Self { data, sp: 0 }
    }

    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        self.data.get_mut(self.sp..self.sp + N).map(|slot| {
            slot.iter_mut().zip(value.iter()).for_each(|(slot, value)| {
                *slot = *value;
            });
            self.sp += N;
        })
    }

    pub fn pop<const N: usize>(&mut self) -> Option<[Word; N]> {
        self.sp.checked_sub(N).and_then(|sp| {
            self.data.get_mut(sp..sp + N).map(|slot| {
                let mut value = [0; N];
                value.iter_mut().zip(slot.iter()).for_each(|(value, slot)| {
                    *value = *slot;
                });
                self.sp = sp;
                value
            })
        })
    }

    pub fn peek<const N: usize>(&self, address: Address) -> Option<&[Word; N]> {
        let address = address as usize;
        if address + N > self.sp {
            None
        } else {
            self.data
                .get(address..address + N)
                .and_then(|slot| slot.try_into().ok())
        }
    }

    pub fn iter(&self, address: Address, len: usize) -> Option<StackIter> {
        let address = address as usize;
        if address + len > self.sp {
            None
        } else {
            self.data
                .get(address..address + len)
                .map(|data| data.iter())
        }
    }

    pub fn move_to(&mut self, to: &mut Stack, len: usize) -> Option<Address> {
        self.sp.checked_sub(len).and_then(|sp| {
            to.data.get_mut(to.sp..to.sp + len).and_then(|slot| {
                self.data.get(sp..sp + len).map(|from| {
                    from.iter().zip(slot.iter_mut()).for_each(|(from, to)| {
                        *to = *from;
                    });
                    let address = to.sp as Address;
                    self.sp = sp;
                    to.sp += len;
                    address
                })
            })
        })
    }
}

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
