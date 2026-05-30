# std-ext

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*std-ext* extends the Rust standard library with additional traits and utilities for common patterns, making standard types more convenient to use.

## Features

- `ArcExt` trait — wrap any value in `Arc` via `.arc()`
- `AtomicExt` trait — convert primitives to atomic types
- `CacheMapExt` — TTL-based cache for HashMap, BTreeMap, DequeBTreeMap, DequeHashMap
- `EntryExt` — extended Entry API with TTL support
- `TimedValue` — value wrapper with expiration
- `OrdExt` / `HashExt` / `OrdHashExt` — ordering and hashing wrappers
- Tuple helper macros: `tuple_deref!`, `tuple_deref_mut!`, `tuple_take!`

## Cargo Feature Flags

This crate has no Cargo feature flags. All functionality is enabled by default.

## Basic Usage

```rust
use std_ext::{ArcExt, AtomicExt};
use std::sync::atomic::Ordering;

// Wrap in Arc
let v = 42.arc();
assert_eq!(*v, 42);

// Convert to atomic
let a = 0usize.atomic();
a.store(10, Ordering::SeqCst);
assert_eq!(a.load(Ordering::SeqCst), 10);
```

## Tests

```bash
cargo test -p std-ext --all-features
```

62 tests covering ArcExt, AtomicExt, CacheMapExt, EntryExt, TimedValue, and wrapper types.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
