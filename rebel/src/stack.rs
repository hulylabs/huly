// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Address, Word};

// S T A C K

pub type StackIter<'a> = std::slice::Iter<'a, Word>;

pub struct Stack<T> {
    data: T,
    sp: usize,
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data, sp: 0 }
    }

    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        self.data
            .as_mut()
            .split_first()
            .map(|(len, slot)| {
                slot.split_first().map(|len, items|{
                    items.iter_mut().zip(value.iter()).for_each(|(item, value)| {
                        *item = *value;
                    });
                    len += N;
            })
    }

    pub fn pop<const N: usize>(&mut self) -> Option<[Word; N]> {
        self.sp.checked_sub(N).and_then(|sp| {
            self.data.as_mut().get_mut(sp..sp + N).map(|slot| {
                let mut value = [0; N];
                value.iter_mut().zip(slot.iter()).for_each(|(value, slot)| {
                    *value = *slot;
                });
                self.sp = sp;
                value
            })
        })
    }
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    pub fn peek<const N: usize>(&self, address: Address) -> Option<&[Word; N]> {
        let address = address as usize;
        if address + N > self.sp {
            None
        } else {
            self.data
                .as_ref()
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
                .as_ref()
                .get(address..address + len)
                .map(|data| data.iter())
        }
    }

    pub fn move_to<U>(&mut self, to: &mut Stack<U>, len: usize) -> Option<Address>
    where
        U: AsMut<[Word]>,
    {
        self.sp.checked_sub(len).and_then(|sp| {
            to.data
                .as_mut()
                .get_mut(to.sp..to.sp + len)
                .and_then(|slot| {
                    self.data.as_ref().get(sp..sp + len).map(|from| {
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
