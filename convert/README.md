# box-convert

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*box-convert* provides type conversion utilities, currently featuring a `Bytesize` type that parses human-readable byte size strings (e.g., "1K2M3B") and formats byte counts back to readable strings.

## Features

- **Bytesize** — human-readable byte size parsing and formatting
- Supports K/M/G suffixes (e.g., "1K", "2M", "3G")
- Combined format parsing (e.g., "1K2M3B")
- Serde support for JSON serialization
- `From<usize>` and `From<&str>` conversions

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `bytesize` | Enable the `Bytesize` type for human-readable byte parsing | no |

## Basic Usage

```rust
use convert::Bytesize;

let b = Bytesize::from("1K");
assert_eq!(*b, 1024);

let b = Bytesize::from(2048usize);
assert_eq!(b.string(), "2K");
```

## Tests

```bash
cargo test -p box-convert --features "bytesize"
```

8 tests covering From implementations, string formatting, serde roundtrip, and edge cases.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
