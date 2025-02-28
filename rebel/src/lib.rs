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

#[macro_export]
macro_rules! rebel2 {
    //==================================================
    // Public entry for a block: [ items... ]
    //==================================================
    ([ $($rest:tt)* ]) => {{
        let vec_items = rebel2!(@parse_block [] $($rest)*);
        $crate::value::Value::Block(vec_items.into_boxed_slice())
    }};

    //==================================================
    // Public entry for a context: { pairs... }
    //==================================================
    ({ $($rest:tt)* }) => {{
        let vec_pairs = rebel2!(@parse_context [] $($rest)*);
        $crate::value::Value::Context(vec_pairs.into_boxed_slice())
    }};

    //==================================================
    // Fallback: parse as an expression => Value::from(expr)
    //==================================================
    ($expr:expr) => {
        $crate::value::Value::from($expr)
    };

    //==================================================
    // Internal: parse block items into accumulator
    //
    //   @parse_block [acc] tokens...
    //
    // Each item can be:
    //   - nested block [ ... ]
    //   - nested context { ... }
    //   - or an expression
    //==================================================
    (@parse_block [$($items:expr),*]) => {
        vec![$($items),*]
    };

    // Match the next token: `[ => parse a nested block
    (@parse_block [$($items:expr),*] [ $($inside:tt)* ] $(, $($rest:tt)*)?) => {
        rebel2!(@parse_block
            [
                $($items,)*
                rebel2!([ $($inside)* ])
            ]
            $($($rest)*)?
        )
    };

    // Match the next token: `{ => parse a nested context
    (@parse_block [$($items:expr),*] { $($inside:tt)* } $(, $($rest:tt)*)?) => {
        rebel2!(@parse_block
            [
                $($items,)*
                rebel2!({ $($inside)* })
            ]
            $($($rest)*)?
        )
    };

    // Otherwise, parse a single expression
    (@parse_block [$($items:expr),*] $expr:expr $(, $($rest:tt)*)?) => {
        rebel2!(@parse_block
            [
                $($items,)*
                $crate::value::Value::from($expr)
            ]
            $($($rest)*)?
        )
    };

    //==================================================
    // Internal: parse context pairs into accumulator
    //
    //   @parse_context [pairs] tokens...
    //
    // Key can be ident or string literal.
    // Value can be:
    //   - nested block [ ... ]
    //   - nested context { ... }
    //   - or an expression
    //==================================================
    (@parse_context [$($pairs:expr),*]) => {
        vec![$($pairs),*]
    };

    // Ident key => nested block
    (@parse_context [$($pairs:expr),*] $key:ident => [ $($inside:tt)* ] $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    stringify!($key).into(),
                    rebel2!([ $($inside)* ])
                )
            ]
            $($($rest)*)?
        )
    };

    // Ident key => nested context
    (@parse_context [$($pairs:expr),*] $key:ident => { $($inside:tt)* } $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    stringify!($key).into(),
                    rebel2!({ $($inside)* })
                )
            ]
            $($($rest)*)?
        )
    };

    // Ident key => expression
    (@parse_context [$($pairs:expr),*] $key:ident => $expr:expr $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    stringify!($key).into(),
                    $crate::value::Value::from($expr)
                )
            ]
            $($($rest)*)?
        )
    };

    // String-literal key => nested block
    (@parse_context [$($pairs:expr),*] $key:literal => [ $($inside:tt)* ] $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    $key.into(),
                    rebel2!([ $($inside)* ])
                )
            ]
            $($($rest)*)?
        )
    };

    // String-literal key => nested context
    (@parse_context [$($pairs:expr),*] $key:literal => { $($inside:tt)* } $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    $key.into(),
                    rebel2!({ $($inside)* })
                )
            ]
            $($($rest)*)?
        )
    };

    // String-literal key => expression
    (@parse_context [$($pairs:expr),*] $key:literal => $expr:expr $(, $($rest:tt)*)?) => {
        rebel2!(@parse_context
            [
                $($pairs,)*
                (
                    $key.into(),
                    $crate::value::Value::from($expr)
                )
            ]
            $($($rest)*)?
        )
    };
}

#[macro_export]
macro_rules! word {
    ($ident:ident) => {
        $crate::value::Value::Word(stringify!($ident).into())
    };
}

#[macro_export]
macro_rules! set_word {
    ($ident:ident) => {
        $crate::value::Value::SetWord(stringify!($ident).into())
    };
}

#[cfg(test)]
mod tests {
    use crate::value::Value;
    use crate::{rebel2, set_word, word}; // Our macros

    #[test]
    fn test_block_negative_numbers() {
        let block = rebel2!([1, -5, true, -999]);
        match block {
            Value::Block(items) => {
                assert_eq!(items.len(), 4);
                assert_eq!(items[0], Value::Int(1));
                assert_eq!(items[1], Value::Int(-5));
                assert_eq!(items[2], Value::Int(1)); // true => 1
                assert_eq!(items[3], Value::Int(-999));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_block_trailing_comma() {
        let block = rebel2!([10, -20, "hello",]);
        match block {
            Value::Block(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Int(10));
                assert_eq!(items[1], Value::Int(-20));
                assert_eq!(items[2], Value::String("hello".into()));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_context_negative() {
        let ctx = rebel2!({
            num => -42,
            flag => true,
            "some-key" => -999
        });
        match ctx {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 3);
                assert_eq!(pairs[0].0, "num");
                assert_eq!(pairs[0].1, Value::Int(-42));
                assert_eq!(pairs[1].0, "flag");
                assert_eq!(pairs[1].1, Value::Int(1));
                assert_eq!(pairs[2].0, "some-key");
                assert_eq!(pairs[2].1, Value::Int(-999));
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_nested_structures() {
        let data = rebel2!({
            nums => [ -1, -2, -3 ],
            sub => {
                active => false,
                nested => [ true, 100, ]
            },
        });

        match data {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 2);

                // Check `nums`
                if let Value::Block(items) = &pairs[0].1 {
                    assert_eq!(items.len(), 3);
                    assert_eq!(items[0], Value::Int(-1));
                    assert_eq!(items[1], Value::Int(-2));
                    assert_eq!(items[2], Value::Int(-3));
                } else {
                    panic!("expected block for `nums`");
                }

                // Check `sub`
                if let Value::Context(subpairs) = &pairs[1].1 {
                    assert_eq!(subpairs.len(), 2);

                    // sub => { active => false, nested => [true, 100] }
                    assert_eq!(subpairs[0].0, "active");
                    assert_eq!(subpairs[0].1, Value::Int(0)); // false => 0

                    if let Value::Block(nested) = &subpairs[1].1 {
                        assert_eq!(nested.len(), 2);
                        assert_eq!(nested[0], Value::Int(1)); // true => 1
                        assert_eq!(nested[1], Value::Int(100));
                    } else {
                        panic!("expected block for `nested`");
                    }
                } else {
                    panic!("expected context for `sub`");
                }
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_word_macros() {
        let w = word!(banana);
        assert_eq!(w, Value::Word("banana".into()));

        let sw = set_word!(apple);
        assert_eq!(sw, Value::SetWord("apple".into()));

        // Combine with rebel2
        let data = rebel2!({
            cmd => (word!(go)),
            target => (set_word!(obj)),
            count => -5
        });
        if let Value::Context(pairs) = data {
            assert_eq!(pairs.len(), 3);
            assert_eq!(pairs[0], ("cmd".into(), Value::Word("go".into())));
            assert_eq!(pairs[1], ("target".into(), Value::SetWord("obj".into())));
            assert_eq!(pairs[2], ("count".into(), Value::Int(-5)));
        } else {
            panic!("expected context");
        }
    }
}
