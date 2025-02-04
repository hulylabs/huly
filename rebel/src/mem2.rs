use std::convert::TryInto;

pub type Word = u32;
pub type Offset = Word;
pub type Symbol = Offset;

/// The core trait that “owns” a block of words.
trait StructMut<T>
where
    Self: Sized,
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize;
    const ITEM_SIZE: usize;

    /// Create an instance from some underlying data.
    fn init(data: T) -> Option<Self>;

    /// Get a mutable reference to the underlying data.
    fn as_mut(&mut self) -> &mut T;

    fn reset(mut self) -> Option<Self> {
        self.as_mut().as_mut().first_mut().map(|hdr| *hdr = 0);
        Some(self)
    }

    /// Allocate a sub-structure `S` from our underlying data.
    ///
    /// In this version the allocated type `S` must be creatable from a
    /// mutable slice (`&'a mut [Word]`).
    fn alloc<'a, S>(&'a mut self, items: usize) -> Option<(Offset, S)>
    where
        T: 'a,
        S: StructMut<&'a mut [Word]>,
    {
        let slice: &mut [Word] = self.as_mut().as_mut();
        let (header, data) = slice.split_first_mut()?;
        let start = *header as usize;
        let end = start + items * S::ITEM_SIZE + S::HDR_SIZE;
        let block = data.get_mut(start..end)?;
        *header = end as Word;
        S::init(block).map(|s| (start as Offset, s))
    }

    /// Allocate a fixed-size array from our underlying data.
    ///
    /// This method allocates exactly `N` words and attempts to convert the
    /// allocated subslice into a `&mut [Word; N]`. No data is copied.
    fn alloc_array<'a, const N: usize>(&'a mut self) -> Option<(Offset, &'a mut [Word; N])>
    where
        T: 'a,
    {
        let slice: &mut [Word] = self.as_mut().as_mut();
        let (header, data) = slice.split_first_mut()?;
        let start = *header as usize;
        let end = start + N;
        let sub = data.get_mut(start..end)?;
        let array_ref: &mut [Word; N] = sub.try_into().ok()?;
        *header = end as Word;
        Some((start as Offset, array_ref))
    }
}

/// A heap-like structure that wraps some memory.
pub struct Heap<T>(T);

impl<T> StructMut<T> for Heap<T>
where
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize = 1;
    const ITEM_SIZE: usize = 1;

    fn init(data: T) -> Option<Self> {
        Self(data).reset()
    }

    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// A stack-like structure that wraps some memory.
pub struct Stack<T>(T);

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    fn push<const N: usize>(&mut self, values: [Word; N]) -> Option<Offset> {
        let (offset, data) = self.alloc_array::<N>()?;
        *data = values;
        Some(offset)
    }
}

impl<T> StructMut<T> for Stack<T>
where
    T: AsMut<[Word]>,
{
    const HDR_SIZE: usize = 1;
    const ITEM_SIZE: usize = 1;

    fn init(data: T) -> Option<Self> {
        Self(data).reset()
    }

    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

// A S S E M B L Y  T E S T S

pub fn test_heap<'a>(heap: &'a mut Heap<&mut [Word]>) -> Option<(Offset, Stack<&'a mut [Word]>)> {
    heap.alloc::<Stack<_>>(16)
}

pub fn test_stack<'a>(stack: &'a mut Stack<&mut [Word]>, a: u32, b: u32) -> Option<(Offset)> {
    //     stack.alloc_array::<2>()
    stack.push([a, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_array_works() {
        let mut mem = [0u32; 64];
        let mut heap = Heap(&mut mem[..]);
        let (offset, arr_ref) = heap.alloc_array::<4>().expect("Allocation failed");
        arr_ref[0] = 42;
        arr_ref[1] = 43;
        arr_ref[2] = 44;
        arr_ref[3] = 45;
        let hdr = heap.as_mut().as_mut()[0];
        assert_eq!(hdr, offset as Word + 4);
    }
}
