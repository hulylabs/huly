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

impl Value {
    //==================================================================
    // CONSTRUCTORS
    //==================================================================

    /// Create a None value
    pub fn none() -> Self {
        Value::None
    }

    /// Create an Int value
    pub fn int(value: i32) -> Self {
        Value::Int(value)
    }

    /// Create a String value
    pub fn string<S: Into<SmolStr>>(value: S) -> Self {
        Value::String(value.into())
    }

    /// Create a Word value
    pub fn word<S: Into<SmolStr>>(value: S) -> Self {
        Value::Word(value.into())
    }

    /// Create a SetWord value
    pub fn set_word<S: Into<SmolStr>>(value: S) -> Self {
        Value::SetWord(value.into())
    }

    /// Create a Block value from any iterable of Values
    pub fn block<I: IntoIterator<Item = Value>>(values: I) -> Self {
        Value::Block(values.into_iter().collect::<Vec<_>>().into_boxed_slice())
    }

    /// Create a Context (object) value from any iterable of key-value pairs
    pub fn context<K: Into<SmolStr>, I: IntoIterator<Item = (K, Value)>>(pairs: I) -> Self {
        Value::Context(
            pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }

    /// Create a Context from a series of key-values using a builder pattern
    pub fn object() -> ContextBuilder {
        ContextBuilder::new()
    }

    /// Create a boolean value (as an Int with value 1 or 0)
    pub fn boolean(value: bool) -> Self {
        Value::Int(if value { 1 } else { 0 })
    }

    //==================================================================
    // TYPE CHECKING
    //==================================================================

    /// Check if value is None
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    /// Check if value is Int
    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }

    /// Check if value is String
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Check if value is Word
    pub fn is_word(&self) -> bool {
        matches!(self, Value::Word(_))
    }

    /// Check if value is SetWord
    pub fn is_set_word(&self) -> bool {
        matches!(self, Value::SetWord(_))
    }

    /// Check if value is Block
    pub fn is_block(&self) -> bool {
        matches!(self, Value::Block(_))
    }

    /// Check if value is Context
    pub fn is_context(&self) -> bool {
        matches!(self, Value::Context(_))
    }

    /// Check if value represents a boolean (Int with value 0 or 1)
    pub fn is_boolean(&self) -> bool {
        match self {
            Value::Int(0 | 1) => true,
            _ => false,
        }
    }

    /// Check if value is truthy (anything except None, Int(0), or empty Block/Context)
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::None => false,
            Value::Int(0) => false,
            Value::Block(block) => !block.is_empty(),
            Value::Context(context) => !context.is_empty(),
            _ => true,
        }
    }

    //==================================================================
    // VALUE EXTRACTION
    //==================================================================

    /// Extract an i32 value if this is an Int
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Extract a string reference if this is a String
    pub fn as_string(&self) -> Option<&SmolStr> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Extract a word reference if this is a Word
    pub fn as_word(&self) -> Option<&SmolStr> {
        match self {
            Value::Word(w) => Some(w),
            _ => None,
        }
    }

    /// Extract a setword reference if this is a SetWord
    pub fn as_set_word(&self) -> Option<&SmolStr> {
        match self {
            Value::SetWord(w) => Some(w),
            _ => None,
        }
    }

    /// Extract a boolean if this is an Int(0) or Int(1)
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Int(0) => Some(false),
            Value::Int(1) => Some(true),
            _ => None,
        }
    }

    /// Extract a block slice if this is a Block
    pub fn as_block(&self) -> Option<&[Value]> {
        match self {
            Value::Block(block) => Some(block),
            _ => None,
        }
    }

    /// Extract a mutable block slice if this is a Block
    pub fn as_block_mut(&mut self) -> Option<&mut [Value]> {
        match self {
            Value::Block(block) => Some(block),
            _ => None,
        }
    }

    /// Extract a context slice if this is a Context
    pub fn as_context(&self) -> Option<&[(SmolStr, Value)]> {
        match self {
            Value::Context(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// Extract a mutable context slice if this is a Context
    pub fn as_context_mut(&mut self) -> Option<&mut [(SmolStr, Value)]> {
        match self {
            Value::Context(pairs) => Some(pairs),
            _ => None,
        }
    }

    //==================================================================
    // CONTEXT OPERATIONS
    //==================================================================

    /// Get a value from a Context using a string key
    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<&Value> {
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

    /// Get a mutable value from a Context using a string key
    pub fn get_mut<K: AsRef<str>>(&mut self, key: K) -> Option<&mut Value> {
        match self {
            Value::Context(pairs) => {
                let key_ref = key.as_ref();
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

    /// Insert a key-value pair into a Context, creating a new Context value
    pub fn insert<K: Into<SmolStr>, V: Into<Value>>(self, key: K, value: V) -> Self {
        match self {
            Value::Context(pairs) => {
                let mut pairs_vec = pairs.to_vec();
                let key = key.into();
                let value = value.into();

                // See if we need to update an existing key
                if let Some(pos) = pairs_vec.iter().position(|(k, _)| k == &key) {
                    pairs_vec[pos] = (key, value);
                } else {
                    pairs_vec.push((key, value));
                }

                Value::Context(pairs_vec.into_boxed_slice())
            }
            // If not a context, create a new one with this key-value pair
            _ => {
                let key = key.into();
                let value = value.into();
                Value::Context(Box::new([(key, value)]))
            }
        }
    }

    /// Remove a key from a Context, returning a new Context value
    pub fn remove<K: AsRef<str>>(self, key: K) -> Self {
        match self {
            Value::Context(pairs) => {
                let key_ref = key.as_ref();
                let mut pairs_vec = pairs.to_vec();
                pairs_vec.retain(|(k, _)| k != key_ref);
                Value::Context(pairs_vec.into_boxed_slice())
            }
            // If not a context, return as-is
            _ => self,
        }
    }

    /// Check if a Context contains a specific key
    pub fn has_key<K: AsRef<str>>(&self, key: K) -> bool {
        self.get(key).is_some()
    }

    /// Get all keys from a Context as a Block value
    pub fn keys(&self) -> Value {
        match self {
            Value::Context(pairs) => {
                let keys = pairs
                    .iter()
                    .map(|(k, _)| Value::String(k.clone()))
                    .collect::<Vec<_>>();
                Value::Block(keys.into_boxed_slice())
            }
            _ => Value::Block(Box::new([])),
        }
    }

    /// Get all values from a Context as a Block value
    pub fn values(&self) -> Value {
        match self {
            Value::Context(pairs) => {
                let values = pairs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
                Value::Block(values.into_boxed_slice())
            }
            _ => Value::Block(Box::new([])),
        }
    }

    //==================================================================
    // BLOCK OPERATIONS
    //==================================================================

    /// Get a value at a specific index from a Block
    pub fn at(&self, index: usize) -> Option<&Value> {
        match self {
            Value::Block(block) => block.get(index),
            _ => None,
        }
    }

    /// Get a mutable value at a specific index from a Block
    pub fn at_mut(&mut self, index: usize) -> Option<&mut Value> {
        match self {
            Value::Block(block) => {
                let block_slice = &mut **block;
                block_slice.get_mut(index)
            }
            _ => None,
        }
    }

    /// Get the length of a Block or Context
    pub fn len(&self) -> usize {
        match self {
            Value::Block(block) => block.len(),
            Value::Context(pairs) => pairs.len(),
            _ => 0,
        }
    }

    /// Check if a Block or Context is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push a value to a Block, returning a new Block value
    pub fn push(self, value: Value) -> Self {
        match self {
            Value::Block(block) => {
                let mut block_vec = block.to_vec();
                block_vec.push(value);
                Value::Block(block_vec.into_boxed_slice())
            }
            // If not a block, create a new one with this value
            _ => Value::Block(Box::new([value])),
        }
    }

    /// Pop a value from a Block, returning a tuple of (new_block, popped_value)
    pub fn pop(self) -> (Self, Option<Value>) {
        match self {
            Value::Block(block) => {
                let mut block_vec = block.to_vec();
                let popped = block_vec.pop();
                (Value::Block(block_vec.into_boxed_slice()), popped)
            }
            // If not a block, return as-is with None
            _ => (self, None),
        }
    }

    /// Map a function over a Block, returning a new Block value
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnMut(Value) -> Value,
    {
        match self {
            Value::Block(block) => {
                let mapped = block.to_vec().into_iter().map(f).collect::<Vec<_>>();
                Value::Block(mapped.into_boxed_slice())
            }
            // If not a block, return as-is
            _ => self,
        }
    }

    /// Filter a Block with a predicate, returning a new Block value
    pub fn filter<F>(self, f: F) -> Self
    where
        F: FnMut(&Value) -> bool,
    {
        match self {
            Value::Block(block) => {
                let filtered = block.to_vec().into_iter().filter(f).collect::<Vec<_>>();
                Value::Block(filtered.into_boxed_slice())
            }
            // If not a block, return as-is
            _ => self,
        }
    }

    //==================================================================
    // PATH OPERATIONS
    //==================================================================

    /// Get a value from a nested path of keys (for Contexts)
    pub fn get_path<I, K>(&self, path: I) -> Option<&Value>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        let mut current = self;
        let mut iter = path.into_iter();

        // Process all path segments except the last one
        while let Some(key) = iter.next() {
            if let Some(next) = current.get(key) {
                if let Some(next_key) = iter.next() {
                    // If there are more segments, continue traversing
                    current = next;
                    match current {
                        Value::Context(_) => {
                            if let Some(next_value) = current.get(next_key) {
                                current = next_value;
                            } else {
                                return None; // Key not found at this level
                            }
                        }
                        _ => return None, // Not a context, can't traverse further
                    }
                } else {
                    // Last segment, return the value
                    return Some(next);
                }
            } else {
                return None; // Key not found
            }
        }

        // If the path is empty, return self
        Some(current)
    }

    /// Set a value at a nested path of keys, creating intermediate contexts as needed
    pub fn set_path<I, K, V>(self, path: I, value: V) -> Self
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str>,
        V: Into<Value>,
    {
        let value = value.into();
        let path_vec: Vec<_> = path.into_iter()
            .map(|k| k.as_ref().to_string())
            .collect();
            
        if path_vec.is_empty() {
            return value; // If path is empty, return the value directly
        }

        // Helper function that works with string slices
        fn set_path_inner(ctx: Value, keys: &[String], value: Value) -> Value {
            if keys.len() == 1 {
                // Single path segment
                if let Value::Context(pairs) = ctx {
                    let key = &keys[0];
                    let mut pairs_vec = pairs.to_vec();
                    
                    // Find existing or add new
                    if let Some(pos) = pairs_vec.iter().position(|(k, _)| k == key) {
                        pairs_vec[pos].1 = value;
                    } else {
                        pairs_vec.push((key.clone().into(), value));
                    }
                    
                    Value::Context(pairs_vec.into_boxed_slice())
                } else {
                    // Create new context
                    let mut pairs = Vec::new();
                    pairs.push((keys[0].clone().into(), value));
                    Value::Context(pairs.into_boxed_slice())
                }
            } else {
                // Multiple path segments - handle recursively
                if let Value::Context(pairs) = ctx {
                    let first_key = &keys[0];
                    let mut pairs_vec = pairs.to_vec();
                    
                    if let Some(pos) = pairs_vec.iter().position(|(k, _)| k == first_key) {
                        // Key exists - update recursively
                        let next_ctx = set_path_inner(
                            pairs_vec[pos].1.clone(),
                            &keys[1..],
                            value
                        );
                        pairs_vec[pos].1 = next_ctx;
                    } else {
                        // Key doesn't exist - create new context
                        let inner_ctx = Value::Context(Box::new([]));
                        let updated = set_path_inner(inner_ctx, &keys[1..], value);
                        pairs_vec.push((first_key.clone().into(), updated));
                    }
                    
                    Value::Context(pairs_vec.into_boxed_slice())
                } else {
                    // Start with a new context
                    let ctx = Value::Context(Box::new([]));
                    set_path_inner(ctx, keys, value)
                }
            }
        }
        
        set_path_inner(self, &path_vec, value)
    }

    //==================================================================
    // CONVERSION UTILITIES
    //==================================================================

    /// Convert value to a string representation
    pub fn to_string_value(&self) -> Value {
        match self {
            Value::None => Value::String("none".into()),
            Value::Int(n) => Value::String(n.to_string().into()),
            Value::String(s) => Value::String(s.clone()),
            Value::Word(w) => Value::String(w.clone()),
            Value::SetWord(w) => Value::String(format!("{}:", w).into()),
            Value::Block(_) => Value::String(format!("{}", self).into()),
            Value::Context(_) => Value::String(format!("{}", self).into()),
        }
    }

    /// Convert a value to an integer if possible
    pub fn to_int_value(&self) -> Value {
        match self {
            Value::Int(n) => Value::Int(*n),
            Value::String(s) => {
                if let Ok(n) = s.parse::<i32>() {
                    Value::Int(n)
                } else {
                    Value::None
                }
            }
            _ => Value::None,
        }
    }

    /// Parse a string into a structured value
    pub fn parse<S: AsRef<str>>(s: S) -> Value {
        // A simple parser could be implemented here
        // For now, just return the string
        Value::String(s.as_ref().into())
    }
}

//==================================================================
// BUILDER PATTERNS
//==================================================================

/// Builder for creating Context values in a fluent style
pub struct ContextBuilder {
    pairs: Vec<(SmolStr, Value)>,
}

impl ContextBuilder {
    /// Create a new empty ContextBuilder
    pub fn new() -> Self {
        ContextBuilder { pairs: Vec::new() }
    }

    /// Add a key-value pair to the context
    pub fn insert<K: Into<SmolStr>, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        self.pairs.push((key.into(), value.into()));
        self
    }

    /// Build the final Context value
    pub fn build(self) -> Value {
        Value::Context(self.pairs.into_boxed_slice())
    }
}

/// Builder for creating Block values in a fluent style
pub struct BlockBuilder {
    values: Vec<Value>,
}

impl BlockBuilder {
    /// Create a new empty BlockBuilder
    pub fn new() -> Self {
        BlockBuilder { values: Vec::new() }
    }

    /// Add a value to the block
    pub fn push<V: Into<Value>>(mut self, value: V) -> Self {
        self.values.push(value.into());
        self
    }

    /// Build the final Block value
    pub fn build(self) -> Value {
        Value::Block(self.values.into_boxed_slice())
    }
}

// Extensions to Value for builders
impl Value {
    /// Start building a block
    pub fn block_builder() -> BlockBuilder {
        BlockBuilder::new()
    }

    /// Start building a context
    pub fn context_builder() -> ContextBuilder {
        ContextBuilder::new()
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
    //===========================================================
    // 1) Top-level: [ ... ] => Block
    //===========================================================
    ([ $($inner:tt)* ]) => {{
        let items = rebel!(@parse_block [] $($inner)*);
        Value::Block(items.into_boxed_slice())
    }};

    //===========================================================
    // 2) Top-level: { ... } => Context with => syntax
    //===========================================================
    ({ $($inner:tt)* }) => {{
        let pairs = rebel!(@parse_context [] $($inner)*);
        Value::Context(pairs.into_boxed_slice())
    }};

    //===========================================================
    // 3) Special case for None
    //===========================================================
    (none) => {
        Value::None
    };

    //===========================================================
    // 4) Fallback single-expr => Value::from(expr)
    //===========================================================
    ($expr:expr) => {
        Value::from($expr)
    };

    //===========================================================
    // BLOCK PARSER: @parse_block [acc] tokens...
    //===========================================================
    // 0) If tokens run out, produce Vec
    (@parse_block [$($items:expr),*]) => (
        vec![$($items),*]
    );

    // 1) optional comma => skip
    (@parse_block [$($items:expr),*] , $($rest:tt)*) => {{
        rebel!(@parse_block [$($items),*] $($rest)*)
    }};

    // 2) sub-block => [ ... ]
    (@parse_block [$($items:expr),*] [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub = rebel!([ $($b)* ]);
        rebel!(@parse_block [$($items,)* sub] $($rest)*)
    }};

    // 3) set-word => ident:
    (@parse_block [$($items:expr),*] $name:ident : $($rest:tt)*) => {{
        let sw = Value::SetWord(stringify!($name).into());
        rebel!(@parse_block [$($items,)* sw] $($rest)*)
    }};

    // 4) string literal
    (@parse_block [$($items:expr),*] $lit:literal $($rest:tt)*) => {{
        let val = match stringify!($lit) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                Value::String(s[1..s.len()-1].into())
            },
            s => match s.parse::<i32>() {
                Ok(n) => Value::Int(n),
                Err(_) => {
                    match s {
                        "true" => Value::Int(1),
                        "false" => Value::Int(0),
                        _ => Value::String(s.into())
                    }
                }
            }
        };
        rebel!(@parse_block [$($items,)* val] $($rest)*)
    }};

    // 5) parenthesized expression => ( ... )
    (@parse_block [$($items:expr),*] ( $($expr:tt)* ) $($rest:tt)*) => {{
        let val_expr = rebel!($($expr)*);
        rebel!(@parse_block [$($items,)* val_expr] $($rest)*)
    }};

    // 6) none keyword
    (@parse_block [$($items:expr),*] none $($rest:tt)*) => {{
        let none_val = Value::None;
        rebel!(@parse_block [$($items,)* none_val] $($rest)*)
    }};

    // 7) bare ident => Word
    (@parse_block [$($items:expr),*] $ident:ident $($rest:tt)*) => {{
        let w = Value::Word(stringify!($ident).into());
        rebel!(@parse_block [$($items,)* w] $($rest)*)
    }};

    //===========================================================
    // CONTEXT PARSER: @parse_context [pairs] tokens...
    //===========================================================
    // 0) if tokens exhausted => produce pairs
    (@parse_context [$($pairs:expr),*]) => (
        vec![$($pairs),*]
    );

    // 1) optional comma => skip
    (@parse_context [$($pairs:expr),*] , $($rest:tt)*) => {{
        rebel!(@parse_context [$($pairs),*] $($rest)*)
    }};

    // 2) sub-block
    (@parse_context [$($pairs:expr),*] $key:ident => [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub_block = rebel!([ $($b)* ]);
        let pair = (stringify!($key).into(), sub_block);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => [ $($b:tt)* ] $($rest:tt)*) => {{
        let sub_block = rebel!([ $($b)* ]);
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let pair = (key_str, sub_block);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 3) sub-context
    (@parse_context [$($pairs:expr),*] $key:ident => { $($inner:tt)* } $($rest:tt)*) => {{
        let sub_ctx = rebel!({ $($inner)* });
        let pair = (stringify!($key).into(), sub_ctx);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => { $($inner:tt)* } $($rest:tt)*) => {{
        let sub_ctx = rebel!({ $($inner)* });
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let pair = (key_str, sub_ctx);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 4) key => ( expr ) => parse
    (@parse_context [$($pairs:expr),*] $key:ident => ( $($expr:tt)* ) $($rest:tt)*) => {{
        let val_expr = rebel!($($expr)*);
        let pair = (stringify!($key).into(), val_expr);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => ( $($expr:tt)* ) $($rest:tt)*) => {{
        let val_expr = rebel!($($expr)*);
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let pair = (key_str, val_expr);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 5) none value
    (@parse_context [$($pairs:expr),*] $key:ident => none $($rest:tt)*) => {{
        let pair = (stringify!($key).into(), Value::None);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => none $($rest:tt)*) => {{
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let pair = (key_str, Value::None);
        rebel!(@parse_context [$($pairs,)* pair] $($rest)*)
    }};

    // 6) fallback expr, splitting into two arms:
    //    (a) key => $val:expr , $($rest:tt)*  => parse next
    //    (b) key => $val:expr               => final pair
    (@parse_context [$($pairs:expr),*] $key:ident => $val:expr , $($rest:tt)*) => {{
        let new_pair = (stringify!($key).into(), Value::from($val));
        rebel!(@parse_context [$($pairs,)* new_pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:ident => $val:expr) => {{
        let new_pair = (stringify!($key).into(), Value::from($val));
        let mut v = vec![$($pairs),*];
        v.push(new_pair);
        v
    }};

    (@parse_context [$($pairs:expr),*] $key:literal => $val:expr , $($rest:tt)*) => {{
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let new_pair = (key_str, Value::from($val));
        rebel!(@parse_context [$($pairs,)* new_pair] $($rest)*)
    }};
    (@parse_context [$($pairs:expr),*] $key:literal => $val:expr) => {{
        let key_str = match stringify!($key) {
            s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
                s[1..s.len()-1].into()
            },
            s => s.into()
        };
        let new_pair = (key_str, Value::from($val));
        let mut v = vec![$($pairs),*];
        v.push(new_pair);
        v
    }};
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_constructors() {
        let none = Value::none();
        let num = Value::int(42);
        let s = Value::string("hello");
        let w = Value::word("print");
        let sw = Value::set_word("count");
        let b = Value::boolean(true);

        assert_eq!(none, Value::None);
        assert_eq!(num, Value::Int(42));
        assert_eq!(s, Value::String("hello".into()));
        assert_eq!(w, Value::Word("print".into()));
        assert_eq!(sw, Value::SetWord("count".into()));
        assert_eq!(b, Value::Int(1));
    }

    #[test]
    fn test_collection_constructors() {
        // Create a block from a vector
        let block = Value::block(vec![Value::int(1), Value::string("two"), Value::none()]);

        // Create a context from pairs
        let context = Value::context(vec![
            ("name", Value::string("John")),
            ("age", Value::int(30)),
        ]);

        // Check block values
        if let Value::Block(items) = &block {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::String("two".into()));
            assert_eq!(items[2], Value::None);
        } else {
            panic!("Not a block!");
        }

        // Check context values
        if let Value::Context(pairs) = &context {
            assert_eq!(pairs.len(), 2);
            assert_eq!(pairs[0].0, "name");
            assert_eq!(pairs[0].1, Value::String("John".into()));
            assert_eq!(pairs[1].0, "age");
            assert_eq!(pairs[1].1, Value::Int(30));
        } else {
            panic!("Not a context!");
        }
    }

    #[test]
    fn test_builder_patterns() {
        // Using the block builder
        let block = Value::block_builder()
            .push(Value::int(1))
            .push("hello") // Conversion from &str
            .push(42) // Conversion from i32
            .build();

        // Using the context builder
        let user = Value::context_builder()
            .insert("name", "Alice") // Key and value are converted
            .insert("age", 30) // Key is &str, value is i32
            .insert("active", true) // Value is bool
            .build();

        // Using the object constructor (shorthand for context builder)
        let config = Value::object()
            .insert("debug", true)
            .insert("timeout", 5000)
            .insert("server", "example.com")
            .build();

        // Verify block
        assert_eq!(block.len(), 3);
        assert_eq!(block.at(0), Some(&Value::Int(1)));

        // Verify user context
        assert!(user.has_key("name"));
        assert_eq!(user.get("age"), Some(&Value::Int(30)));

        // Verify config
        assert_eq!(config.get("timeout"), Some(&Value::Int(5000)));
    }

    #[test]
    fn test_type_checking() {
        let int_val = Value::int(42);
        let str_val = Value::string("hello");
        let none_val = Value::none();
        let bool_val = Value::boolean(true);

        assert!(int_val.is_int());
        assert!(!int_val.is_string());

        assert!(str_val.is_string());
        assert!(!str_val.is_int());

        assert!(none_val.is_none());

        assert!(bool_val.is_int());
        assert!(bool_val.is_boolean());
        assert_eq!(bool_val.as_boolean(), Some(true));
    }

    #[test]
    fn test_context_operations() {
        // Create a context
        let mut user = Value::object()
            .insert("name", "Bob")
            .insert("age", 25)
            .build();

        // Get values
        assert_eq!(user.get("name"), Some(&Value::String("Bob".into())));

        // Modify a value
        if let Some(age) = user.get_mut("age") {
            *age = Value::int(26);
        }
        assert_eq!(user.get("age"), Some(&Value::Int(26)));

        // Insert a new key (returning a new context)
        let user = user.insert("email", "bob@example.com");
        assert!(user.has_key("email"));

        // Remove a key
        let user = user.remove("age");
        assert!(!user.has_key("age"));

        // Get keys and values
        let keys = user.keys();
        if let Value::Block(items) = keys {
            assert_eq!(items.len(), 2);
            // Keys should include "name" and "email" (order may vary)
        }
    }

    #[test]
    fn test_block_operations() {
        // Create a block
        let block = Value::block(vec![Value::int(1), Value::int(2), Value::int(3)]);

        // Test length
        assert_eq!(block.len(), 3);

        // Access items
        assert_eq!(block.at(1), Some(&Value::Int(2)));

        // Push a value (returns a new block)
        let block = block.push(Value::int(4));
        assert_eq!(block.len(), 4);

        // Pop a value
        let (block, popped) = block.pop();
        assert_eq!(block.len(), 3);
        assert_eq!(popped, Some(Value::Int(4)));

        // Map operation
        let doubled = block.map(|v| {
            if let Value::Int(n) = v {
                Value::Int(n * 2)
            } else {
                v
            }
        });

        if let Value::Block(items) = doubled {
            assert_eq!(items[0], Value::Int(2));
            assert_eq!(items[1], Value::Int(4));
            assert_eq!(items[2], Value::Int(6));
        }

        // Filter operation
        let block = Value::block(vec![
            Value::int(1),
            Value::int(2),
            Value::int(3),
            Value::int(4),
        ]);

        let evens = block.filter(|v| {
            if let Value::Int(n) = v {
                n % 2 == 0
            } else {
                false
            }
        });

        if let Value::Block(items) = evens {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Value::Int(2));
            assert_eq!(items[1], Value::Int(4));
        }
    }

    #[test]
    fn test_path_operations() {
        // Create a nested context
        let data = Value::object()
            .insert(
                "user",
                Value::object()
                    .insert(
                        "profile",
                        Value::object()
                            .insert("name", "Charlie")
                            .insert("email", "charlie@example.com")
                            .build(),
                    )
                    .insert(
                        "settings",
                        Value::object()
                            .insert("theme", "dark")
                            .insert("notifications", true)
                            .build(),
                    )
                    .build(),
            )
            .build();

        // Get a value using path
        let name = data.get_path(["user", "profile", "name"]);
        assert_eq!(name, Some(&Value::String("Charlie".into())));

        // Set a value using path (returns a new value)
        let updated = data
            .clone()
            .set_path(["user", "settings", "language"], "en");

        // Verify the update
        let language = updated.get_path(["user", "settings", "language"]);
        assert_eq!(language, Some(&Value::String("en".into())));

        // Create a deep path that doesn't exist yet
        let with_new_path = data
            .clone()
            .set_path(["user", "preferences", "colors", "primary"], "#3366FF");

        // Verify the deep path was created
        let color = with_new_path.get_path(["user", "preferences", "colors", "primary"]);
        assert_eq!(color, Some(&Value::String("#3366FF".into())));
    }

    #[test]
    fn test_conversions() {
        // String conversions
        let int_val = Value::int(42);
        let int_as_str = int_val.to_string_value();
        assert_eq!(int_as_str, Value::String("42".into()));

        // Int conversions
        let str_val = Value::string("42");
        let str_as_int = str_val.to_int_value();
        assert_eq!(str_as_int, Value::Int(42));

        // Invalid conversion
        let bad_str = Value::string("hello");
        let bad_int = bad_str.to_int_value();
        assert_eq!(bad_int, Value::None);
    }

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

        // Negative integer - requires parentheses
        let v2: Value = rebel!(-42);
        assert_eq!(v2, Value::Int(-42));

        // Expression
        let num = 42;
        let v3: Value = rebel!(num);
        assert_eq!(v3, Value::Int(42));
        
        // Integers in a block
        let v4 = rebel!([ 1 2 3 -4 -5 ]);
        if let Value::Block(items) = v4 {
            assert_eq!(items.len(), 5);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
            assert_eq!(items[3], Value::Int(-4));
            assert_eq!(items[4], Value::Int(-5));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_string_values() {
        // String literals
        let v1: Value = rebel!("hello");
        assert_eq!(v1, Value::String("hello".into()));

        // String from variable
        let s = "test".to_string();
        let v2: Value = rebel!(s);
        assert_eq!(v2, Value::String("test".into()));
        
        // String in a block
        let v3 = rebel!([ "hello" "world" ]);
        if let Value::Block(items) = v3 {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Value::String("hello".into()));
            assert_eq!(items[1], Value::String("world".into()));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_word_values() {
        // Word in a block
        let v1 = rebel!([ alpha beta gamma ]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Word("alpha".into()));
            assert_eq!(items[1], Value::Word("beta".into())); 
            assert_eq!(items[2], Value::Word("gamma".into()));
        } else {
            panic!("Expected block");
        }

        // SetWord in a block
        let v2 = rebel!([ x: y: z: ]);
        if let Value::Block(items) = v2 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::SetWord("x".into()));
            assert_eq!(items[1], Value::SetWord("y".into()));
            assert_eq!(items[2], Value::SetWord("z".into()));
        } else {
            panic!("Expected block");
        }

        // Direct Value construction
        let v3 = Value::Word("apple".into());
        assert_eq!(v3, Value::Word("apple".into()));
        
        let v4 = Value::SetWord("banana".into());
        assert_eq!(v4, Value::SetWord("banana".into()));
    }
    
    // Tests migrated from the rebel4 macro:
    
    #[test]
    fn test_new_block_basics() {
        // Word, string, setword, nested block
        let b = rebel!([ alpha "hello" x: [a b] ]);
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
    fn test_new_block_negative() {
        // Must do ( -5 )
        let b = rebel!([ foo ( -5 ) bar ]);
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
    fn test_new_context_no_commas() {
        let name = "Alice";
        let c = rebel!({
            user => name,
            count => 42,
            nested => [ x y z ]
        });
        match c {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 3);
                assert_eq!(pairs[0].0, "user");
                assert_eq!(pairs[0].1, Value::String("Alice".into()));
                assert_eq!(pairs[1].0, "count");
                assert_eq!(pairs[1].1, Value::Int(42));
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
    fn test_new_context_comma() {
        let c = rebel!({
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
    fn test_new_context_negative_expr() {
        // negative => -999
        let base = 40;
        let c = rebel!({
            answer => (base + 2),
            negative => -999
        });
        match c {
            Value::Context(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "answer");
                assert_eq!(pairs[0].1, Value::Int(42));
                assert_eq!(pairs[1].0, "negative");
                assert_eq!(pairs[1].1, Value::Int(-999));
            }
            _ => panic!("expected context"),
        }
    }

    #[test]
    fn test_new_fallback_expr() {
        let x = rebel!(10 + 5);
        assert_eq!(x, Value::Int(15));

        let s = "hi";
        let val2 = rebel!(s);
        assert_eq!(val2, Value::String("hi".into()));
    }

    #[test]
    fn test_new_fallback_negative() {
        let v = rebel!(-5);
        assert_eq!(v, Value::Int(-5));
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

        // Context with standard arrow syntax
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

        // Context with identifier keys
        let v3: Value = rebel!({ name => "John", age => 42 });
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
        let v1: Value = rebel!([ 1 [2 3] 4 ]);
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
            user => {
                name => "John",
                profile => {
                    age => 42
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
    fn test_string_formatting() {
        // We can use format! directly with Value::from
        let name = "World";
        let v1 = Value::String(format!("Hello, {}!", name).into());
        assert_eq!(v1, Value::String("Hello, World!".into()));

        let count = 42;
        let v2 = Value::String(format!("Count: {}", count).into());
        assert_eq!(v2, Value::String("Count: 42".into()));
    }

    #[test]
    fn test_nested_contexts() {
        // Create a context with nested structure
        let user_info = rebel!({
            user => { 
                name => "John",
                email => "john@example.com",
                settings => {
                    theme => "dark",
                    notifications => true
                }
            }
        });

        // Check values at various levels
        if let Value::Context(pairs) = &user_info {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "user");
            
            // Get user context
            if let Value::Context(user) = &pairs[0].1 {
                // Check first level properties
                assert_eq!(user.len(), 3);
                
                let name = user[0].1.clone();
                let email = user[1].1.clone();
                
                assert_eq!(name, Value::String("John".into()));
                assert_eq!(email, Value::String("john@example.com".into()));
                
                // Check settings
                if let Value::Context(settings) = &user[2].1 {
                    assert_eq!(settings[0].0, "theme");
                    assert_eq!(settings[0].1, Value::String("dark".into()));
                    
                    assert_eq!(settings[1].0, "notifications");
                    assert_eq!(settings[1].1, Value::Int(1)); // true becomes 1
                }
            }
        }
        
        // Use get_path (existing functionality) to traverse paths
        let name = user_info.get_path(["user", "name"]);
        assert_eq!(name, Some(&Value::String("John".into())));
        
        let theme = user_info.get_path(["user", "settings", "theme"]);
        assert_eq!(theme, Some(&Value::String("dark".into())));
    }

    #[test]
    fn test_boolean_values() {
        // Booleans convert to integers (1 and 0)
        let v1: Value = rebel!(true);
        assert_eq!(v1, Value::Int(1));

        let v2: Value = rebel!(false);
        assert_eq!(v2, Value::Int(0));

        // In contexts
        let v3: Value = rebel!({ active => true });
        if let Value::Context(items) = v3 {
            assert_eq!(items[0].1, Value::Int(1));
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_complex_example() {
        // Create subcontexts first
        let variant1 = rebel!({
            id => 1,
            sku => "ABC-123",
            inStock => true,
            features => [ "wireless", "bluetooth" ]
        });
        
        let variant2 = rebel!({
            id => 2,
            sku => "ABC-456",
            inStock => false
        });
        
        // Create main context with the variants as a block
        let complex = rebel!({
            id => 1001,
            name => "Product",
            tags => [ electronics, sale ],
            variants => (Value::block(vec![variant1, variant2]))
        });

        // Validate the structure
        if let Value::Context(_items) = &complex {
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
                if let Value::Context(_v1) = &variants[0] {
                    let sku = variants[0].get("sku").unwrap();
                    assert_eq!(*sku, Value::String("ABC-123".into()));

                    // Features
                    if let Some(Value::Block(features)) = variants[0].get("features") {
                        assert_eq!(features.len(), 2);
                        assert_eq!(features[0], Value::String("wireless".into()));
                    }
                }
            }
        }
    }

    #[test]
    fn test_minimal_external_variables() {
        let name_str = "Alice";
        let age_num = 30;

        // Direct variable use - forces expression pattern
        let v1 = Value::string(name_str);
        let v2 = rebel!(name_str.to_string()); // Method call is an expression

        assert_eq!(v1, Value::String("Alice".into()));
        assert_eq!(v2, Value::String("Alice".into()));

        // In context
        let ctx1 = rebel!({
            "key1" => (name_str),  // Parentheses force expression pattern
            "key2" => (age_num)    // Parentheses force expression pattern
        });

        assert_eq!(ctx1.get("key1"), Some(&Value::String("Alice".into())));
        assert_eq!(ctx1.get("key2"), Some(&Value::Int(30)));

        // Pre-built values always work
        let tag_values = Value::block(vec![Value::string("user"), Value::string("premium")]);

        let ctx2 = rebel!({
            "name" => (name_str),
            "tags" => (tag_values)
        });

        assert_eq!(ctx2.get("name"), Some(&Value::String("Alice".into())));
    }

    #[test]
    fn test_comprehensive_external_variables() {
        // Define test variables of different types
        let name = "Alice";
        let age = 30;
        let is_active = true;
        let tags = vec!["user", "premium"];

        //==============================================================
        // 1. DIRECT VARIABLE USAGE
        //==============================================================

        // Direct variable reference in a block - creates Word values with variable name
        let block_with_words = rebel!([ name age is_active ]);
        
        // Extract and check the words
        if let Value::Block(items) = block_with_words {
            assert_eq!(items[0], Value::Word("name".into()));
            assert_eq!(items[1], Value::Word("age".into()));
            assert_eq!(items[2], Value::Word("is_active".into()));
        }

        // To use the variable's value directly, convert it to a string or use an expression
        let v4 = Value::string(name); // Now this uses the variable value
        assert_eq!(v4, Value::String(name.into()));

        //==============================================================
        // 2. CONTEXT WITH IDENTIFIER KEYS AND ARROW SYNTAX
        //==============================================================

        // Using arrow syntax
        let user1 = rebel!({
            name => "name",
            age => "age",
            active => "is_active"
        });

        // Check the keys and values
        assert_eq!(user1.get("name"), Some(&Value::String("name".into())));
        assert_eq!(user1.get("age"), Some(&Value::String("age".into())));
        assert_eq!(user1.get("active"), Some(&Value::String("is_active".into())));

        //==============================================================
        // 3. USING EXTERNAL VARIABLES WITH VALUES
        //==============================================================

        // Convert variables to Values directly
        let age_val = Value::int(age);
        let active_val = Value::boolean(is_active);

        let user2 = rebel!({
            name => (name.to_string()),  // Convert directly here
            age => (age_val),            // Uses the int value
            active => (active_val)       // Uses the boolean value
        });

        assert_eq!(user2.get("name"), Some(&Value::String(name.into())));
        assert_eq!(user2.get("age"), Some(&Value::Int(age)));
        assert_eq!(user2.get("active"), Some(&Value::Int(1)));

        //==============================================================
        // 4. HANDLING COLLECTIONS/ARRAYS
        //==============================================================

        // 4.1 Pre-create a Block Value (recommended approach)
        let tag_block = Value::block(vec![Value::string(tags[0]), Value::string(tags[1])]);

        let user3 = rebel!({
            name => (name.to_string()),
            tags => (tag_block)   // Pre-created Block works correctly
        });

        if let Some(Value::Block(block)) = user3.get("tags") {
            assert_eq!(block.len(), 2);
            assert_eq!(block[0], Value::String(tags[0].into()));
            assert_eq!(block[1], Value::String(tags[1].into()));
        } else {
            panic!("Tags should be a block");
        }

        // 4.2 Convert to Vec<Value> (alternative approach)
        let tag_vec: Vec<Value> = tags.iter().map(|&t| Value::string(t)).collect();

        let user4 = rebel!({
            name => (name.to_string()),
            tags => (tag_vec)    // Vec<Value> works correctly
        });

        if let Some(Value::Block(block)) = user4.get("tags") {
            assert_eq!(block.len(), 2);
            assert_eq!(block[0], Value::String(tags[0].into()));
            assert_eq!(block[1], Value::String(tags[1].into()));
        } else {
            panic!("Tags should be a block");
        }

        // 4.3 Direct literals work fine (when variables not needed)
        let user5 = rebel!({
            name => (name.to_string()),
            tags => [ "user", "premium" ]   // String literals work directly
        });

        if let Some(Value::Block(block)) = user5.get("tags") {
            assert_eq!(block.len(), 2);
            assert_eq!(block[0], Value::String("user".into()));
            assert_eq!(block[1], Value::String("premium".into()));
        } else {
            panic!("Tags should be a block");
        }

        //==============================================================
        // 5. ADVANCED PATTERNS
        //==============================================================

        // Using a function result directly works
        let product = Value::object()
            .insert("name", "Product")
            .insert("price", 1999)
            .build();

        let basket = rebel!({
            customer => (name.to_string()),
            item => (product.clone())    // Using a Value directly works
        });

        assert_eq!(basket.get("customer"), Some(&Value::String(name.into())));
        assert_eq!(basket.get("item"), Some(&product));
    }
}
