// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;
use std::convert::From;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Int(i32),
    Block(Box<[Value]>),
    String(SmolStr),
    Word(SmolStr),
    SetWord(SmolStr),
    Context(Box<[(SmolStr, Value)]>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "none"),
            Value::Int(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Word(w) => write!(f, "{}", w),
            Value::SetWord(w) => write!(f, "{}:", w),
            Value::Block(block) => {
                write!(f, "[")?;
                let mut first = true;
                for item in block.iter() {
                    if !first {
                        write!(f, " ")?;
                    }
                    first = false;
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Context(pairs) => {
                write!(f, "{{ ")?;
                let mut first = true;
                for (key, value) in pairs.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, " }}")
            }
        }
    }
}

// Fix conflicting implementations by using trait objects instead of generic parameters
// for indexing contexts with strings
impl Value {
    // Access methods with explicit types
    pub fn get<S: AsRef<str>>(&self, key: S) -> Option<&Value> {
        match self {
            Value::Context(pairs) => {
                let key_ref = key.as_ref();
                pairs
                    .iter()
                    .find_map(|(k, v)| if k == key_ref { Some(v) } else { None })
            }
            _ => None,
        }
    }

    pub fn get_mut<S: AsRef<str>>(&mut self, key: S) -> Option<&mut Value> {
        match self {
            Value::Context(pairs) => {
                let key_ref = key.as_ref();
                // We need to get a mutable reference to the boxed slice
                let pairs_slice = &mut **pairs;
                for (k, v) in pairs_slice.iter_mut() {
                    if k == key_ref {
                        return Some(v);
                    }
                }
                None
            }
            _ => None,
        }
    }

    // Get a value at a numeric index from a Block
    pub fn at(&self, index: usize) -> Option<&Value> {
        match self {
            Value::Block(block) => block.get(index),
            _ => None,
        }
    }

    pub fn at_mut(&mut self, index: usize) -> Option<&mut Value> {
        match self {
            Value::Block(block) => {
                let block_slice = &mut **block;
                block_slice.get_mut(index)
            }
            _ => None,
        }
    }

    // Method to convert a Rust i32 to a Value::Int
    pub fn from_int(i: i32) -> Self {
        Value::Int(i)
    }

    // Method to convert a Rust string to a Value::String
    pub fn from_string<S: Into<SmolStr>>(s: S) -> Self {
        Value::String(s.into())
    }

    // Method to convert a Rust string to a Value::Word
    pub fn from_word<S: Into<SmolStr>>(s: S) -> Self {
        Value::Word(s.into())
    }

    // Method to convert a Rust string to a Value::SetWord
    pub fn from_set_word<S: Into<SmolStr>>(s: S) -> Self {
        Value::SetWord(s.into())
    }

    // Method to convert a Rust bool to a Value::Int (since we don't have a Boolean type)
    pub fn from_bool(b: bool) -> Self {
        Value::Int(if b { 1 } else { 0 })
    }
}

// From implementations for common types
impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value.into())
    }
}

impl From<SmolStr> for Value {
    fn from(value: SmolStr) -> Self {
        Value::String(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Int(if value { 1 } else { 0 })
    }
}

impl From<Vec<Value>> for Value {
    fn from(values: Vec<Value>) -> Self {
        Value::Block(values.into_boxed_slice())
    }
}

// The kitchen sink macro with improved type handling
#[macro_export]
macro_rules! rebel {
    //=============================================
    // NONE
    //=============================================
    (none) => {
        $crate::value::Value::None
    };

    //=============================================
    // LITERAL HANDLING (numbers, strings, booleans)
    //=============================================
    // Match literals directly to handle special cases explicitly
    ($i:literal) => {{
        const LITERAL_VAL: &'static str = stringify!($i);
        // Special case for true/false literals
        if LITERAL_VAL == "true" {
            $crate::value::Value::Int(1)
        } else if LITERAL_VAL == "false" {
            $crate::value::Value::Int(0)
        }
        // Handle integer literals
        else if let Ok(n) = LITERAL_VAL.parse::<i32>() {
            $crate::value::Value::Int(n)
        }
        // Handle string literals by removing the quotes
        else if LITERAL_VAL.starts_with('"') && LITERAL_VAL.ends_with('"') && LITERAL_VAL.len() >= 2 {
            $crate::value::Value::String(LITERAL_VAL[1..LITERAL_VAL.len()-1].into())
        }
        // Other literals
        else {
            $crate::value::Value::String(LITERAL_VAL.into())
        }
    }};

    //=============================================
    // EXPLICIT TYPES
    //=============================================
    // Explicit int
    (int: $i:expr) => {
        $crate::value::Value::Int($i)
    };

    // Explicit string - make sure to handle quoted strings properly
    (string: $s:expr) => {{
        let s = $s;
        let s_str = s.to_string(); // Convert any type to String
        $crate::value::Value::String(s_str.into())
    }};

    // Explicit word
    (word: $w:expr) => {
        $crate::value::Value::Word($w.into())
    };

    // Explicit setword
    (set: $w:expr) => {
        $crate::value::Value::SetWord($w.into())
    };

    //=============================================
    // WORD SYNTAX
    //=============================================
    // Word with identifier (bare identifier for word)
    ($w:ident) => {
        $crate::value::Value::Word(stringify!($w).into())
    };

    // SetWord with identifier (using set() function-like syntax)
    (set($w:ident)) => {
        $crate::value::Value::SetWord(stringify!($w).into())
    };

    // Word with explicit type (still keep this for compatibility)
    (word: $w:expr) => {
        $crate::value::Value::Word($w.into())
    };

    // SetWord with explicit type (still keep this for compatibility)
    (set: $w:expr) => {
        $crate::value::Value::SetWord($w.into())
    };

    //=============================================
    // BLOCK
    //=============================================
    // Empty block
    ([]) => {
        $crate::value::Value::Block(Box::new([]))
    };

    // Block with values (manually handle each element type)
    ([ $($val:tt),* $(,)? ]) => {
        {
            let mut values = Vec::new();
            $(
                values.push(rebel!(@handle_value $val));
            )*
            $crate::value::Value::Block(values.into_boxed_slice())
        }
    };

    //=============================================
    // CONTEXT
    //=============================================
    // Empty context
    ({}) => {
        $crate::value::Value::Context(Box::new([]))
    };

    // Context with key-value pairs (string keys)
    ({ $($key:expr => $val:tt),* $(,)? }) => {
        {
            let mut pairs = Vec::new();
            $(
                // Handle quoted string keys
                let k_str = stringify!($key);
                let k = if k_str.starts_with('"') && k_str.ends_with('"') && k_str.len() >= 2 {
                    k_str[1..k_str.len()-1].into()
                } else {
                    $key.into()
                };
                let v = rebel!(@handle_value $val);
                pairs.push((k, v));
            )*
            $crate::value::Value::Context(pairs.into_boxed_slice())
        }
    };

    // Context with key-value pairs (identifier keys)
    ({ $($key:ident: $val:tt),* $(,)? }) => {
        {
            let mut pairs = Vec::new();
            $(
                let k = stringify!($key).into();
                let v = rebel!(@handle_value $val);
                pairs.push((k, v));
            )*
            $crate::value::Value::Context(pairs.into_boxed_slice())
        }
    };

    //=============================================
    // ADVANCED FEATURES
    //=============================================
    // Template with substitution
    (template: $template:expr, { $($key:ident => $val:tt),* $(,)? }) => {
        {
            let mut template = $template.to_string();
            $(
                let placeholder = format!("{{{}}}", stringify!($key));
                // Convert the value to string without extra quotes
                let value = match rebel!(@handle_value $val) {
                    $crate::value::Value::String(s) => s.to_string(),
                    $crate::value::Value::Int(i) => i.to_string(),
                    $crate::value::Value::None => "none".to_string(),
                    $crate::value::Value::Word(w) => w.to_string(),
                    $crate::value::Value::SetWord(w) => format!("{}:", w),
                    v => format!("{}", v),
                };
                template = template.replace(&placeholder, &value);
            )*
            $crate::value::Value::String(template.into())
        }
    };

    // Path-based update (simplified to work more reliably)
    (path: $base:expr, [$($key:expr),+] = $val:tt) => {
        {
            let mut ctx = $base.clone();
            // Handle string literals in the keys
            let keys: Vec<String> = vec![$(
                {
                    let k_str = stringify!($key);
                    if k_str.starts_with('"') && k_str.ends_with('"') && k_str.len() >= 2 {
                        k_str[1..k_str.len()-1].to_string()
                    } else {
                        $key.to_string()
                    }
                }
            ),+];
            let value = rebel!(@handle_value $val);

            // Use a helper function for the path update
            $crate::value::set_path_value(&mut ctx, &keys, value)
        }
    };

    //=============================================
    // INTERNAL HELPERS
    //=============================================
    // Handle different value types internally
    (@handle_value none) => { $crate::value::Value::None };
    (@handle_value true) => { $crate::value::Value::Int(1) };
    (@handle_value false) => { $crate::value::Value::Int(0) };
    (@handle_value $i:literal) => {{
        const LITERAL_VAL: &'static str = stringify!($i);
        // Special case for true/false literals
        if LITERAL_VAL == "true" {
            $crate::value::Value::Int(1)
        } else if LITERAL_VAL == "false" {
            $crate::value::Value::Int(0)
        }
        // Handle integer literals
        else if let Ok(n) = LITERAL_VAL.parse::<i32>() {
            $crate::value::Value::Int(n)
        }
        // Handle string literals by removing the quotes
        else if LITERAL_VAL.starts_with('"') && LITERAL_VAL.ends_with('"') && LITERAL_VAL.len() >= 2 {
            $crate::value::Value::String(LITERAL_VAL[1..LITERAL_VAL.len()-1].into())
        }
        // Other literals
        else {
            $crate::value::Value::String(LITERAL_VAL.into())
        }
    }};
    (@handle_value $w:ident) => { $crate::value::Value::Word(stringify!($w).into()) };
    (@handle_value set($w:ident)) => { $crate::value::Value::SetWord(stringify!($w).into()) };
    (@handle_value [ $($val:tt),* $(,)? ]) => { rebel!([ $($val),* ]) };
    (@handle_value { $($key:expr => $val:tt),* $(,)? }) => { rebel!({ $($key => $val),* }) };
    (@handle_value { $($key:ident: $val:tt),* $(,)? }) => { rebel!({ $($key: $val),* }) };
    (@handle_value $other:expr) => { $crate::value::Value::from($other) };

    //=============================================
    // FALLTHROUGH
    //=============================================
    // For direct expressions with explicit type conversion
    ($expr:expr) => {
        $crate::value::Value::from($expr)
    };
}

// Standalone helper function to support path updates
pub fn set_path_value(ctx: &mut Value, keys: &[impl AsRef<str>], value: Value) -> Value {
    // Top level context handling
    if keys.len() == 1 {
        if let Value::Context(pairs) = ctx {
            let key = keys[0].as_ref();
            let mut pairs_vec = pairs.to_vec();

            // Find existing or add new
            let found = pairs_vec.iter_mut().position(|(k, _)| k == key);
            if let Some(pos) = found {
                pairs_vec[pos].1 = value;
            } else {
                pairs_vec.push((key.into(), value));
            }

            *ctx = Value::Context(pairs_vec.into_boxed_slice());
            return ctx.clone();
        } else {
            // Create new context if current value is not a context
            let mut pairs = Vec::new();
            pairs.push((keys[0].as_ref().into(), value));
            return Value::Context(pairs.into_boxed_slice());
        }
    }

    // Handle nested paths
    if let Value::Context(pairs) = ctx {
        let first_key = keys[0].as_ref();
        let mut pairs_vec = pairs.to_vec();

        // Find or create the nested context
        let found = pairs_vec.iter_mut().position(|(k, _)| k == first_key);
        if let Some(pos) = found {
            // Update the existing path
            let next_keys = &keys[1..];
            let mut next_ctx = pairs_vec[pos].1.clone();
            pairs_vec[pos].1 = set_path_value(&mut next_ctx, next_keys, value);
        } else {
            // Create a new nested path
            let mut inner_ctx = Value::Context(Box::new([]));
            inner_ctx = set_path_value(&mut inner_ctx, &keys[1..], value);
            pairs_vec.push((first_key.into(), inner_ctx));
        }

        *ctx = Value::Context(pairs_vec.into_boxed_slice());
        return ctx.clone();
    } else {
        // Create a new context structure
        let mut current = Value::Context(Box::new([]));
        current = set_path_value(&mut current, keys, value);
        return current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_value() {
        let v = rebel!(none);
        assert_eq!(v, Value::None);
    }

    #[test]
    fn test_integer_values() {
        // Direct integer literals
        let v1: Value = rebel!(42);
        assert_eq!(v1, Value::Int(42));

        // Negative integer
        let v2: Value = rebel!(-42);
        assert_eq!(v2, Value::Int(-42));

        // Explicit type
        let v3: Value = rebel!(int: 42);
        assert_eq!(v3, Value::Int(42));

        // Expression
        let num = 42;
        let v4: Value = rebel!(int: num);
        assert_eq!(v4, Value::Int(42));
    }

    #[test]
    fn test_string_values() {
        // String literals
        let v1: Value = rebel!("hello");
        assert_eq!(v1, Value::String("hello".into()));

        // Explicit string
        let v2: Value = rebel!(string: "world");
        assert_eq!(v2, Value::String("world".into()));

        // String with explicit type conversion
        let s = "test".to_string();
        let v3: Value = rebel!(string: s);
        assert_eq!(v3, Value::String("test".into()));
    }

    #[test]
    fn test_word_values() {
        // Word with explicit type
        let v1: Value = rebel!(word: "apple");
        assert_eq!(v1, Value::Word("apple".into()));

        // Word with bare identifier
        let v2: Value = rebel!(apple);
        assert_eq!(v2, Value::Word("apple".into()));

        // SetWord with explicit type
        let v3: Value = rebel!(set: "apple");
        assert_eq!(v3, Value::SetWord("apple".into()));

        // SetWord with function-like syntax
        let v4: Value = rebel!(set(apple));
        assert_eq!(v4, Value::SetWord("apple".into()));
    }

    #[test]
    fn test_block_values() {
        // Empty block
        let v1: Value = rebel!([]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 0);
        } else {
            panic!("Not a block");
        }

        // Block with integers
        let v2: Value = rebel!([1, 2, 3]);
        if let Value::Block(items) = v2 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
        } else {
            panic!("Not a block");
        }

        // Block with mixed types
        let v3: Value = rebel!([1, "two", none]);
        if let Value::Block(items) = v3 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::String("two".into()));
            assert_eq!(items[2], Value::None);
        } else {
            panic!("Not a block");
        }
    }

    #[test]
    fn test_context_values() {
        // Empty context
        let v1: Value = rebel!({});
        if let Value::Context(items) = v1 {
            assert_eq!(items.len(), 0);
        } else {
            panic!("Not a context");
        }

        // Context with standard syntax
        let v2: Value = rebel!({ "name" => "John", "age" => 42 });
        if let Value::Context(items) = v2 {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].0, "name");
            assert_eq!(items[0].1, Value::String("John".into()));
            assert_eq!(items[1].0, "age");
            assert_eq!(items[1].1, Value::Int(42));
        } else {
            panic!("Not a context");
        }

        // Context with object-like syntax
        let v3: Value = rebel!({ name: "John", age: 42 });
        if let Value::Context(items) = v3 {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].0, "name");
            assert_eq!(items[0].1, Value::String("John".into()));
            assert_eq!(items[1].0, "age");
            assert_eq!(items[1].1, Value::Int(42));
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_nested_structures() {
        // Nested blocks
        let v1: Value = rebel!([1, [2, 3], 4]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[2], Value::Int(4));

            if let Value::Block(inner) = &items[1] {
                assert_eq!(inner.len(), 2);
                assert_eq!(inner[0], Value::Int(2));
                assert_eq!(inner[1], Value::Int(3));
            } else {
                panic!("Inner item is not a block");
            }
        } else {
            panic!("Not a block");
        }

        // Nested contexts
        let v2: Value = rebel!({
            user: {
                name: "John",
                profile: {
                    age: 42
                }
            }
        });

        if let Value::Context(items) = v2 {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].0, "user");

            if let Value::Context(user) = &items[0].1 {
                assert_eq!(user.len(), 2);
                assert_eq!(user[0].0, "name");
                assert_eq!(user[0].1, Value::String("John".into()));
            } else {
                panic!("User is not a context");
            }
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_template_syntax() {
        let v1: Value = rebel!(template: "Hello, {name}!", {
            name => "World"
        });

        assert_eq!(v1, Value::String("Hello, World!".into()));

        let v2: Value = rebel!(template: "Count: {count}", {
            count => 42
        });

        assert_eq!(v2, Value::String("Count: 42".into()));
    }

    #[test]
    fn test_path_expressions() {
        // Start with a simple context
        let base: Value = rebel!({ user: { name: "John" } });

        // Update existing path
        let v1 = rebel!(path: base, ["user", "name"] = "Jane");

        if let Value::Context(pairs) = &v1 {
            let user = pairs[0].1.get("name").unwrap();
            assert_eq!(user, &Value::String("Jane".into()));
        }

        // Add new path
        let v2 = rebel!(path: base, ["user", "email"] = "john@example.com");

        if let Value::Context(pairs) = &v2 {
            let user = pairs[0].1.get("email").unwrap();
            assert_eq!(user, &Value::String("john@example.com".into()));
        }
    }

    #[test]
    fn test_boolean_values() {
        // Booleans convert to integers (1 and 0)
        let v1: Value = rebel!(true);
        assert_eq!(v1, Value::Int(1));

        let v2: Value = rebel!(false);
        assert_eq!(v2, Value::Int(0));

        // In contexts
        let v3: Value = rebel!({ active: true });
        if let Value::Context(items) = v3 {
            assert_eq!(items[0].1, Value::Int(1));
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_complex_example() {
        let complex: Value = rebel!({
            id: 1001,
            name: "Product",
            tags: [ electronics, sale ],
            variants: [
                {
                    id: 1,
                    sku: "ABC-123",
                    inStock: true,
                    features: [ "wireless", "bluetooth" ]
                },
                {
                    id: 2,
                    sku: "ABC-456",
                    inStock: false
                }
            ]
        });

        // Validate the structure
        if let Value::Context(items) = &complex {
            // Check id
            let id = complex.get("id").unwrap();
            assert_eq!(*id, Value::Int(1001));

            // Check tags
            if let Some(Value::Block(tags)) = complex.get("tags") {
                assert_eq!(tags.len(), 2);
                assert_eq!(tags[0], Value::Word("electronics".into()));
            }

            // Check variants
            if let Some(Value::Block(variants)) = complex.get("variants") {
                assert_eq!(variants.len(), 2);

                // First variant
                if let Value::Context(v1) = &variants[0] {
                    let sku = variants[0].get("sku").unwrap();
                    assert_eq!(*sku, Value::String("ABC-123".into()));

                    // Features
                    if let Some(Value::Block(features)) = variants[0].get("features") {
                        assert_eq!(features.len(), 2);
                        assert_eq!(features[0], "wireless".into());
                    }
                }
            }
        }
    }
}
