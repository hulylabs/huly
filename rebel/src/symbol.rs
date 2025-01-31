// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

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

    pub fn hash(&self) -> u32 {
        crate::hash::hash(&self.buf)
    }
}
