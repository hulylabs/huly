#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Int(i32),
    String(String),
    Word(String),
    SetWord(String),
    Block(Box<[Value]>),
    Context(Box<[(String, Value)]>),
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Int(n)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Int(if b { 1 } else { 0 })
    }
}

#[macro_export]
macro_rules! rebel4 {
    //===========================================================
    // 1) Top-level: [ ... ] => Block
    //===========================================================
    ([ $($inner:tt)* ]) => {{
        let items = rebel4!(@parse_block [] $($inner)*);
        Value::Block(items.into_boxed_slice())
    }};

    //===========================================================
    // 2) Top-level: { ... } => Context
    //===========================================================
    ({ $($inner:tt)* }) => {{
        let pairs = rebel4!(@parse_context [] $($inner)*);
        Value::Context(pairs.into_boxed_slice())
    }};

    //===========================================================
    // 3) Fallback single-expr => Value::from(expr)
    //===========================================================
    ($expr:expr) => {
        Value::from($expr)
    };

    //===========================================================
    // BLOCK PARSER: @parse_block [acc] tokens...
    // No literal ']' match => no mismatched delimiter
    //===========================================================
    // 0) If tokens run out, produce Vec
    (@parse_block [$($items:expr),*]) => (
        vec![$($items),*]
    );

    // 1) optional comma => skip
    (@parse_block [$($items:expr),*] , $($rest:tt)*) => {{
        rebel4!(@parse_block [$($items),*] $($rest)*)
    }};

    // 2) sub-block => [ ... ]
    (@parse_block [$($items:expr),*] [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub = rebel4!([ $($b)* ]);
        rebel4!(@parse_block [$($items,)* sub] $($rest)*)
    }};

    // 3) set-word => ident:
    (@parse_block [$($items:expr),*] $name:ident : $($rest:tt)*) => {{
        let sw = Value::SetWord(stringify!($name).into());
        rebel4!(@parse_block [$($items,)* sw] $($rest)*)
    }};

    // 4) string literal
    (@parse_block [$($items:expr),*] $lit:literal $($rest:tt)*) => {{
        let val_str = Value::String($lit.to_string());
        rebel4!(@parse_block [$($items,)* val_str] $($rest)*)
    }};

    // 5) parenthesized expression => ( ... )
    (@parse_block [$($items:expr),*] ( $($expr:tt)* ) $($rest:tt)*) => {{
        let val_expr = rebel4!($($expr)*);
        rebel4!(@parse_block [$($items,)* val_expr] $($rest)*)
    }};

    // 6) bare ident => Word
    (@parse_block [$($items:expr),*] $ident:ident $($rest:tt)*) => {{
        let w = Value::Word(stringify!($ident).into());
        rebel4!(@parse_block [$($items,)* w] $($rest)*)
    }};

    //===========================================================
    // CONTEXT PARSER: @parse_context [pairs] tokens...
    // We'll parse pairs until no more tokens or top-level ends.
    //===========================================================
    // 0) if tokens exhausted => produce pairs
    (@parse_context [$($pairs:expr),*]) => (
        vec![$($pairs),*]
    );

    // 1) optional comma => skip
    (@parse_context [$($pairs:expr),*] , $($rest:tt)*) => {{
        rebel4!(@parse_context [$($pairs),*] $($rest)*)
    }};

    // 2) sub-block
    (@parse_context [$($pairs:expr),*] $key:ident => [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub_block = rebel4!([ $($b)* ]);
        let pair = (stringify!($key).to_string(), sub_block);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub_block = rebel4!([ $($b)* ]);
        let pair = ($key.to_string(), sub_block);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 3) sub-context
    (@parse_context [$($pairs:expr),*] $key:ident => { $($inner:tt)* } $($rest:tt)*) => {{
        let sub_ctx = rebel4!({ $($inner)* });
        let pair = (stringify!($key).to_string(), sub_ctx);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => { $($inner:tt)* } $($rest:tt)*) => {{
        let sub_ctx = rebel4!({ $($inner)* });
        let pair = ($key.to_string(), sub_ctx);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 4) key => ( expr ) => parse
    (@parse_context [$($pairs:expr),*] $key:ident => ( $($expr:tt)* ), $($rest:tt)*) => {{
        let val_expr = rebel4!($($expr)*);
        let pair = (stringify!($key).to_string(), val_expr);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:ident => ( $($expr:tt)* )) => {{
        let val_expr = rebel4!($($expr)*);
        let pair = (stringify!($key).to_string(), val_expr);
        let mut v = vec![$($pairs),*];
        v.push(pair);
        v
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => ( $($expr:tt)* ), $($rest:tt)*) => {{
        let val_expr = rebel4!($($expr)*);
        let pair = ($key.to_string(), val_expr);
        rebel4!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => ( $($expr:tt)* )) => {{
        let val_expr = rebel4!($($expr)*);
        let pair = ($key.to_string(), val_expr);
        let mut v = vec![$($pairs),*];
        v.push(pair);
        v
    }};

    // 5) fallback expr, splitting into two arms:
    //    (a) key => $val:expr , $($rest:tt)*  => parse next
    //    (b) key => $val:expr               => final pair
    (@parse_context [$($pairs:expr),*] $key:ident => $val:expr , $($rest:tt)*) => {{
        let new_pair = (stringify!($key).to_string(), Value::from($val));
        rebel4!(@parse_context [$($pairs,)* new_pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:ident => $val:expr) => {{
        let new_pair = (stringify!($key).to_string(), Value::from($val));
        let mut v = vec![$($pairs),*];
        v.push(new_pair);
        v
    }};

    (@parse_context [$($pairs:expr),*] $key:literal => $val:expr , $($rest:tt)*) => {{
        let new_pair = ($key.to_string(), Value::from($val));
        rebel4!(@parse_context [$($pairs,)* new_pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => $val:expr) => {{
        let new_pair = ($key.to_string(), Value::from($val));
        let mut v = vec![$($pairs),*];
        v.push(new_pair);
        v
    }};
}

#[cfg(test)]
mod tests {
    use super::{rebel4, Value};

    #[test]
    fn test_block_basics() {
        // Word, string, setword, nested block
        let b = rebel4!([ alpha "hello" x: [a b] ]);
        match b {
            Value::Block(items) => {
                assert_eq!(items.len(), 4);
                assert_eq!(items[0], Value::Word("alpha".into()));
                assert_eq!(items[1], Value::String("hello".into()));

                // x:
                if let Value::SetWord(sw) = &items[2] {
                    assert_eq!(sw, "x");
                } else if let Value::Block(_) = &items[2] {
                    panic!("We must check carefully. Maybe the nested block is item #2?");
                } else {
                    panic!("expected setword or block");
                }
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_block_negative() {
        // Must do ( -5 )
        let b = rebel4!([ foo ( -5 ) bar ]);
        match b {
            Value::Block(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Word("foo".into()));
                assert_eq!(items[1], Value::Int(-5));
                assert_eq!(items[2], Value::Word("bar".into()));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_context_no_commas() {
        let name = "Alice";
        let c = rebel4!({
            user => name,
            count => 42,
            nested => [ x y z ]
        });
        match c {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 3);
                assert_eq!(pairs[0], ("user".into(), Value::String("Alice".into())));
                assert_eq!(pairs[1], ("count".into(), Value::Int(42)));
                if let Value::Block(b) = &pairs[2].1 {
                    assert_eq!(b.len(), 3);
                } else {
                    panic!("expected block");
                }
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_context_comma() {
        let c = rebel4!({
            one => 1,
            two => 2,
        });
        match c {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 2);
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_context_negative_expr() {
        // negative => -999
        let base = 40;
        let c = rebel4!({
            answer => (base + 2),
            negative => -999
        });
        match c {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "answer");
                assert_eq!(pairs[0].1, Value::Int(42));
                assert_eq!(pairs[1], ("negative".to_string(), Value::Int(-999)));
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_fallback_expr() {
        let x = rebel4!(10 + 5);
        assert_eq!(x, Value::Int(15));

        let s = "hi";
        let val2 = rebel4!(s);
        assert_eq!(val2, Value::String("hi".into()));
    }

    #[test]
    fn test_fallback_negative() {
        let v = rebel4!(-5);
        assert_eq!(v, Value::Int(-5));
    }
}
