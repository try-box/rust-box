# mpsc

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*mpsc* provides Multiple Producer Single Consumer channels built on top of `queue-ext`. Supports multiple backing queue implementations, each with different characteristics.

## Features

- **SegQueue** channel — lock-free, crossbeam-based (default)
- **VecDeque** channel — standard Rust VecDeque backed
- **Priority** channel — items prioritized by key order
- **IndexMap** channel — deduplication by key (last write wins)
- Thread-safe `Sender`/`Receiver` types
- Single-threaded `LocalSender`/`LocalReceiver` variants
- `Sink` and `Stream` trait implementations
- Type-erased channel API

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `segqueue` | Lock-free channel via `crossbeam::SegQueue` | **yes** |
| `vecdeque` | Channel backed by `std::collections::VecDeque` | no |
| `priority` | Priority channel via `box-collections::PriorityQueue` | no |
| `indexmap` | Deduplicating channel via `indexmap::IndexMap` | no |

## Basic Usage

```rust
use mpsc::segqueue_channel;
use futures::executor::block_on;

let (mut tx, mut rx) = segqueue_channel(10);
block_on(tx.send(42)).unwrap();
assert_eq!(block_on(rx.recv()), Some(42));
```

## More Examples

- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)

## Tests

```bash
cargo test -p mpsc --features "segqueue,vecdeque,priority,indexmap"
```

37 tests covering all 4 channel types, LocalSender/Receiver, clone, and drop semantics.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
