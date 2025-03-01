// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

mod boot;
pub mod collector;
pub mod core;
pub mod encoding;
mod hash;
pub mod mem;
pub mod parse;
pub mod serialize;
pub mod value;


#[cfg(test)]
mod tests {
    // All tests have been moved to value.rs module tests
}
