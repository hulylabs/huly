# Binary Serialization Format

## Overview

RebelDB uses a compact binary serialization format for efficiently storing and transmitting `Value` types. The format is designed to be:

1. Compact with minimal overhead
2. Self-describing with type information preserved
3. Consistent with the RebelDB virtual machine's internal representation
4. Efficient for serialization and deserialization operations

This document describes the binary format in detail.

## Format Structure

Each serialized value consists of:

1. A type tag byte (matches Tag constants in core.rs)
2. Type-specific data (varies by value type)

### Type Tags

The following tags are used to identify value types:

| Tag Value | Type            | Description                     |
|-----------|----------------|---------------------------------|
| 0         | None           | Represents no value             |
| 1         | Int            | Integer value                   |
| 2         | Block          | Sequence of values              |
| 3         | Context        | Key-value pairs                 |
| 5         | String         | UTF-8 string                    |
| 6         | Word           | Reference word                  |
| 7         | SetWord        | Word with assignment marker     |

### Type-Specific Encoding

#### None

```
[TAG_NONE]
```

Simply the tag byte with no additional data.

#### Int

```
[TAG_INT][varint-encoded integer]
```

The integer value is encoded using variable-length encoding (see below).

#### String, Word, SetWord

```
[TAG][varint-encoded length][UTF-8 bytes]
```

These types all follow the same pattern:
1. Tag byte (TAG_INLINE_STRING, TAG_WORD, or TAG_SET_WORD)
2. Length of the string in bytes as a varint
3. The raw UTF-8 bytes of the string content

#### Block

```
[TAG_BLOCK][varint-encoded length][serialized value 1]...[serialized value n]
```

1. Tag byte
2. Number of elements in the block as a varint
3. Each contained value serialized recursively

#### Context

```
[TAG_CONTEXT][varint-encoded length][key-value pairs...]
```

Where each key-value pair is:
```
[varint-encoded key length][UTF-8 bytes of key][serialized value]
```

1. Tag byte
2. Number of key-value pairs as a varint
3. For each pair:
   - Length of the key string as a varint
   - UTF-8 bytes of the key string
   - Serialized value (recursively encoded)

## Variable-Length Integer Encoding

RebelDB uses a compact variable-length encoding for integers that is optimized for small values. The encoding scheme works as follows:

### Small Positive Values (0-63)

Values 0 through 63 are encoded in a single byte with the high bit unset (0xxxxxxx).

### Small Negative Values (-1 to -64)

Values -1 through -64 are encoded in a single byte with the high bit set (1xxxxxxx), where the lower 7 bits represent the negative offset from -1.

### Larger Values

For larger values, a tag byte is used followed by 1-4 bytes of data:

1. One byte (values 64-127 or -65 to -128):
   - Positive: `[0x40][value byte]`
   - Negative: `[0x44][abs(value) byte]`

2. Two bytes (values 128-32767 or -129 to -32768):
   - Positive: `[0x41][high byte][low byte]`
   - Negative: `[0x45][high byte][low byte]`

3. Three bytes (values 32768-8388607 or -32769 to -8388608):
   - Positive: `[0x42][high byte][middle byte][low byte]`
   - Negative: `[0x46][high byte][middle byte][low byte]`

4. Four bytes (values 8388608-2147483647 or -8388609 to -2147483648):
   - Positive: `[0x43][byte 3][byte 2][byte 1][byte 0]`
   - Negative: `[0x47][byte 3][byte 2][byte 1][byte 0]`

This encoding ensures small integers (which are common) use minimal space, while still supporting the full i32 range.

## Example

Let's encode the following value:
```
Value::Context([
    ("name", Value::String("John")),
    ("age", Value::Int(30))
])
```

Binary representation (shown in hex):
```
03       # TAG_CONTEXT
02       # Length: 2 pairs

04       # Key length: 4 
6E616D65 # UTF-8 "name"
05       # TAG_INLINE_STRING
04       # Length: 4
4A6F686E # UTF-8 "John"

03       # Key length: 3
616765   # UTF-8 "age"
01       # TAG_INT
1E       # 30 (varint-encoded)
```

## API Usage

The serialization system provides simple high-level functions:

```rust
// Serialization
let bytes: Vec<u8> = to_bytes(&value)?;

// Deserialization
let value: Value = from_bytes(&bytes)?;
```

For streaming or custom I/O, you can use the lower-level API:

```rust
// Serialization to a writer
let mut serializer = BinarySerializer::new(writer);
value.serialize(&mut serializer)?;

// Deserialization from a reader
let mut deserializer = BinaryDeserializer::new(reader);
let value = deserializer.read_value()?;
```

## Error Handling

The serialization system uses strongly-typed errors to handle various failure cases:

- `IoError`: Underlying I/O operation failed
- `InvalidIntegerEncoding`: Integer could not be decoded
- `InvalidUtf8`: String data is not valid UTF-8
- `NegativeLength`: Negative length value encountered
- `InvalidTag`: Unknown type tag byte
- `UnexpectedEnd`: Premature end of input

## Implementation

The binary serialization format is implemented using the Visitor pattern:

1. The `Serializer` trait defines operations for serializing different types
2. `BinarySerializer` implements this trait for the binary format
3. The `ValueSerialize` trait extension enables `value.serialize(serializer)` syntax
4. `BinaryDeserializer` handles reading the format back into Values

This design allows for future alternative serialization formats (like JSON or MessagePack) by implementing new Serializers without changing the `Value` type itself.