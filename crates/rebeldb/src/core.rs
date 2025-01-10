// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// core.rs:

pub type Hash = [u8; 32];
pub type InlineBytes = [u8; 37];
pub type Inline = (u8, InlineBytes);

#[derive(Debug)]
pub enum Content {
    Inline(Inline),
    Hash(Hash),
}

#[derive(Debug)]
pub enum Value {
    None,

    Uint(u32),
    Int(i32),
    Float(f32),
    Uint64(u64),
    Int64(i64),
    Float64(f64),

    PubKey(Hash),

    String(Content),

    SetWord(Inline),
    GetWord(Inline),
    LitWord(Inline),

    Block(Box<[Value]>),
    // Context(Box<[(Cav<'a>, Value<'a>)]>),
}
