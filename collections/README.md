# box-collections

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*box-collections* provides `no_std` compatible wrappers around `alloc::collections::BinaryHeap`, implementing `BinaryHeap` and `PriorityQueue` with extended functionality like sorted iteration, drained sorted, and optional serde support.

## Features

- **BinaryHeap** — priority queue with sorted iteration and draining
- **PriorityQueue** — priority-value pairs with custom ordering
- `no_std` compatible (requires `alloc`)
- Optional serde support
- `iter_sorted()`, `drain_sorted()`, `into_sorted_vec()` methods

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `std` | Use Rust standard library (`std`) | **yes** |
| `binary-heap` | Enable the `BinaryHeap` wrapper | no |
| `priority-queue` | Enable the `PriorityQueue` wrapper | no |
| `serde` | Enable serialization/deserialization support | no |

## Basic Usage

```rust
use collections::PriorityQueue;

let mut pq = PriorityQueue::new();
pq.push(3, "low");
pq.push(1, "high");
pq.push(2, "mid");

assert_eq!(pq.pop(), Some((1, "high")));
assert_eq!(pq.peek(), Some((&2, &"mid")));
```

## Tests

```bash
cargo test -p box-collections --features "binary-heap,priority-queue,serde"
```

18 tests covering empty, single, duplicate elements, sorted iteration, and serde roundtrip.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
