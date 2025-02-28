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
    pub fn set_path<I, K, V>(mut self, path: I, value: V) -> Self
    where
        I: IntoIterator<Item = K>,
        K: AsRef<str> + Into<SmolStr>,
        V: Into<Value>,
    {
        let path_vec: Vec<K> = path.into_iter().collect();
        if path_vec.is_empty() {
            return value.into(); // If path is empty, return the value directly
        }

        set_path_value(&mut self, &path_vec, value.into())
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
