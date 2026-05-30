# dequemap

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*dequemap* provides a double-ended queue with map features. It combines a `VecDeque` index with a `BTreeMap` or `HashMap` backend, allowing push/pop from both ends while maintaining key-value access.

Supports `no_std` environments with `alloc`.

## Features

- **DequeBTreeMap** — ordered map with double-ended queue operations
- **DequeHashMap** — hash map with double-ended queue operations
- Entry API (VacantEntry / OccupiedEntry) similar to std's map entries
- `no_std` compatible
- Optional serde support
- Push front/back, pop front/back, remove, retain, clear

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `std` | Use Rust standard library (`std`) | **yes** |
| `btreemap` | Enable `DequeBTreeMap` (ordered backend) | **yes** |
| `hashmap` | Enable `DequeHashMap` (hash backend, pulls in `hashbrown`) | no |
| `serde` | Enable serialization/deserialization support | no |

## Basic Usage

```rust
use dequemap::DequeBTreeMap;

let mut map = DequeBTreeMap::new();
map.push_back("a", 1);
map.push_back("b", 2);
map.push_front("c", 3);

assert_eq!(map.front(), Some((&"c", &3)));
assert_eq!(map.back(), Some((&"b", &2)));
assert_eq!(map.pop_front(), Some(("c", 3)));
assert_eq!(map.len(), 2);
```

## Tests

```bash
cargo test -p dequemap --features "btreemap,hashmap,serde"
```

30 tests covering both backends, Entry API, serde roundtrip, large entries, and edge cases.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
