// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

mod boot;
pub mod collector;
pub mod core;
pub mod encoding;
mod hash;
mod mem;
pub mod parse;
pub mod serialize;
pub mod value;

/// The rebel2! macro
#[macro_export]
macro_rules! rebel2 {
    //============================================================
    // TOP-LEVEL MATCHES
    //============================================================

    // 1) [ ... ] => parse a BLOCK
    ([ $($rest:tt)* ]) => {{
        let items = $crate::rebel2!(@parse_block [] $($rest)*);
        $crate::Value::Block(items.into_boxed_slice())
    }};

    // 2) { ... } => parse a CONTEXT
    ({ $($rest:tt)* }) => {{
        let pairs = $crate::rebel2!(@parse_context [] $($rest)*);
        $crate::Value::Context(pairs.into_boxed_slice())
    }};

    // 3) fallback expression => Value::from(expr)
    ($expr:expr) => {
        $crate::Value::from($expr)
    };

    //============================================================
    // BLOCK PARSER: @parse_block [acc] tokenstream...
    //
    // We'll parse until we see a literal ']', ignoring optional commas.
    // Items can be:
    //   - sub-block [ ... ]
    //   - set-word ident:
    //   - string literal "..."
    //   - parenthesized expression ( ... )
    //   - bare identifier => Word(...)
    // Negative numbers must be `( -5 )` in a block.
    //============================================================

    // A) End of block if we see ']'
    (@parse_block [$($items:expr),*]) ] => {
        vec![$($items),*]
    };

    // B) If out of tokens, produce what we have
    (@parse_block [$($items:expr),*]) => {
        vec![$($items),*]
    };

    // C) Optional comma -> ignore
    (@parse_block [$($items:expr),*] , $($rest:tt)*) => {
        $crate::rebel2!(@parse_block [$($items),*] $($rest)*)
    };

    // 1) Sub-block: next token is '['
    (@parse_block [$($items:expr),*] [ $($inside:tt)* ] $($rest:tt)*) => {
        let sub = $crate::rebel2!([ $($inside)* ]);
        $crate::rebel2!(@parse_block [$($items,)* sub] $($rest)*)
    };

    // 2) Set-word => ident:
    (@parse_block [$($items:expr),*] $name:ident : $($rest:tt)*) => {
        $crate::rebel2!(@parse_block
            [
                $($items,)*
                $crate::Value::SetWord(stringify!($name).into())
            ]
            $($rest)*
        )
    };

    // 3) String literal
    (@parse_block [$($items:expr),*] $s:literal $($rest:tt)*) => {
        $crate::rebel2!(@parse_block
            [
                $($items,)*
                $s.into()
            ]
            $($rest)*
        )
    };

    // 4) Parenthesized expr => parse as Rust expression
    (@parse_block [$($items:expr),*] ( $($code:tt)* ) $($rest:tt)*) => {
        let val = $crate::rebel2!( $($code)* );
        $crate::rebel2!(@parse_block [$($items,)* val] $($rest)*)
    };

    // 5) Bare identifier => Word(...)
    (@parse_block [$($items:expr),*] $ident:ident $($rest:tt)*) => {
        $crate::rebel2!(@parse_block
            [
                $($items,)*
                $crate::Value::Word(stringify!($ident).into())
            ]
            $($rest)*
        )
    };

    //============================================================
    // CONTEXT PARSER: @parse_context [pairs] tokenstream...
    //
    // We'll parse until we see '}', ignoring commas.
    // Each pair is `key => value`.
    // Key can be ident or "literal".
    // The value can be:
    //   - sub-block [ ... ]
    //   - sub-context { ... }
    //   - parenthesized expr ( ... )
    //   - fallback expr => e.g. -5, my_var, false, etc.
    //============================================================

    // A) End of context if we see '}'
    (@parse_context [$($pairs:expr),*]) } => {
        vec![$($pairs),*]
    };

    // B) No more tokens => produce what we have
    (@parse_context [$($pairs:expr),*]) => {
        vec![$($pairs),*]
    };

    // C) optional comma => ignore it
    (@parse_context [$($pairs:expr),*] , $($rest:tt)*) => {
        $crate::rebel2!(@parse_context [$($pairs),*] $($rest)*)
    };

    // 1) ident key => sub-block
    (@parse_context [$($pairs:expr),*] $key:ident => [ $($block:tt)* ] $($rest:tt)*) => {
        let sub = $crate::rebel2!([ $($block)* ]);
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                (stringify!($key).into(), sub)
            ]
            $($rest)*
        )
    };

    // 1b) string-literal key => sub-block
    (@parse_context [$($pairs:expr),*] $key:literal => [ $($block:tt)* ] $($rest:tt)*) => {
        let sub = $crate::rebel2!([ $($block)* ]);
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                ($key.into(), sub)
            ]
            $($rest)*
        )
    };

    // 2) ident key => sub-context
    (@parse_context [$($pairs:expr),*] $key:ident => { $($inside:tt)* } $($rest:tt)*) => {
        let sub = $crate::rebel2!({ $($inside)* });
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                (stringify!($key).into(), sub)
            ]
            $($rest)*
        )
    };

    // 2b) string-literal key => sub-context
    (@parse_context [$($pairs:expr),*] $key:literal => { $($inside:tt)* } $($rest:tt)*) => {
        let sub = $crate::rebel2!({ $($inside)* });
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                ($key.into(), sub)
            ]
            $($rest)*
        )
    };

    // 3) ident key => ( ... ) => parse as Rust expr
    (@parse_context [$($pairs:expr),*] $key:ident => ( $($code:tt)* ) $($rest:tt)*) => {
        let val = $crate::rebel2!( $($code)* );
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                (stringify!($key).into(), val)
            ]
            $($rest)*
        )
    };

    // 3b) string-literal key => ( ... )
    (@parse_context [$($pairs:expr),*] $key:literal => ( $($code:tt)* ) $($rest:tt)*) => {
        let val = $crate::rebel2!( $($code)* );
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                ($key.into(), val)
            ]
            $($rest)*
        )
    };

    // 4) ident key => fallback expr
    (@parse_context [$($pairs:expr),*] $key:ident => $val:expr $($rest:tt)*) => {
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                (stringify!($key).into(), $crate::Value::from($val))
            ]
            $($rest)*
        )
    };

    // 4b) string-literal key => fallback expr
    (@parse_context [$($pairs:expr),*] $key:literal => $val:expr $($rest:tt)*) => {
        $crate::rebel2!(@parse_context
            [
                $($pairs,)*
                ($key.into(), $crate::Value::from($val))
            ]
            $($rest)*
        )
    };
}

// ===================================
// Some tests
// ===================================
#[cfg(test)]
mod tests {
    use super::Value;
    use super::super::rebel2; // Or just `use crate::rebel2;` if in same crate

    #[test]
    fn test_block_no_commas() {
        const MY_VAR: i32 = 42;
        let b = rebel2!([ func x "hello" x: (MY_VAR) [a b] ]);
        match b {
            Value::Block(outer) => {
                // Expect 6 items
                assert_eq!(outer.len(), 6);
                assert_eq!(outer[0], Value::Word("func".into()));
                assert_eq!(outer[1], Value::Word("x".into()));
                assert_eq!(outer[2], Value::String("hello".into()));
                assert_eq!(outer[3], Value::SetWord("x".into()));
                assert_eq!(outer[4], Value::Int(42));
                if let Value::Block(nested) = &outer[5] {
                    assert_eq!(nested.len(), 2);
                    assert_eq!(nested[0], Value::Word("a".into()));
                    assert_eq!(nested[1], Value::Word("b".into()));
                } else {
                    panic!("expected nested block");
                }
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_block_negative() {
        // Must do `( -5 )` in a block
        let b = rebel2!([ x ( -5 ) y ]);
        match b {
            Value::Block(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Word("x".into()));
                assert_eq!(items[1], Value::Int(-5));
                assert_eq!(items[2], Value::Word("y".into()));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_context_no_commas() {
        let name = "Alice".to_string();
        let ctx = rebel2!({
            user => name
            count => -99
            sub => [foo bar]
        });
        match ctx {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 3);
                // user => "Alice"
                assert_eq!(pairs[0].0, "user");
                assert_eq!(pairs[0].1, Value::String("Alice".into()));

                // count => -99
                assert_eq!(pairs[1].0, "count");
                assert_eq!(pairs[1].1, Value::Int(-99));

                // sub => block
                assert_eq!(pairs[2].0, "sub");
                if let Value::Block(blk) = &pairs[2].1 {
                    assert_eq!(blk.len(), 2);
                    assert_eq!(blk[0], Value::Word("foo".into()));
                    assert_eq!(blk[1], Value::Word("bar".into()));
                } else {
                    panic!("expected block for sub");
                }
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_context_block_and_expr() {
        let val = rebel2!({
            greeting => "hello"
            data => [ a b c ]
            number => (10 + 5)
        });
        match val {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 3);
                // greeting => "hello"
                assert_eq!(pairs[0].0, "greeting");
                assert_eq!(pairs[0].1, Value::String("hello".into()));

                // data => [ Word("a"), Word("b"), Word("c") ]
                assert_eq!(pairs[1].0, "data");
                if let Value::Block(b) = &pairs[1].1 {
                    assert_eq!(b.len(), 3);
                    assert_eq!(b[0], Value::Word("a".into()));
                    assert_eq!(b[1], Value::Word("b".into()));
                    assert_eq!(b[2], Value::Word("c".into()));
                } else {
                    panic!("expected block for data");
                }

                // number => 15
                assert_eq!(pairs[2].0, "number");
                assert_eq!(pairs[2].1, Value::Int(15));
            }
            _ => panic!("expected context"),
        }
    }
}
