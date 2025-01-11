// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// blob.rs:

pub type Hash = [u8; 32];

pub trait Heap {
    fn put(&mut self, data: &[u8]) -> Hash;
}
