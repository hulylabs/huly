// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// module.rs:

use crate::eval::NativeFn;
use linkme::distributed_slice;

pub struct Module {
    pub name: &'static str,
    pub functions: &'static [(&'static str, NativeFn)],
}

#[distributed_slice]
pub static MODULES: [Module];
