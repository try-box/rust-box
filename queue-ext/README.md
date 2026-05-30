# queue-ext

<!-- Badges placeholder -->
<!-- Github CI, Crates.io, Docs.rs badges -->

English | [简体中文](./README-CN.md)

*queue-ext* provides extended features for queues, allowing conversion of the `pop()` function to a Stream and the `push()` function to a Sender. It serves as a foundational building block for async channel implementations.

## Features

- Convert queue `pop()` into a `Stream` implementation
- Convert queue `push()` into a `Sink`/`Sender` implementation
- Type-erased channel API via `queue_channel()`
- `QueueExt` trait for easy integration with any queue type
- `SendError` type with `full` and `disconnected` variants
- Supports arbitrary queue backends (SegQueue, VecDeque, etc.)
- Thread-safe (`Send` + `Sync`)

## Cargo Feature Flags

This crate has no Cargo feature flags. All functionality is enabled by default.

## Basic Usage

```rust
use queue_ext::{QueueExt, Action, Reply};
use std::task::Poll;
use std::sync::Arc;
use crossbeam_queue::SegQueue;

let queue = Arc::new(SegQueue::new());
let (mut tx, mut rx) = queue.queue_channel::<i32, _, _, _>(
    |s, act| match act {
        Action::Send(val) => { s.push(val); Reply::Send(()) }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
        Action::Len => Reply::Len(s.len()),
    },
    |s, _| match s.pop() {
        Some(m) => Poll::Ready(Some(m)),
        None => Poll::Pending,
    },
);

tx.try_send(42).unwrap();
assert_eq!(rx.is_closed(), false);
```

## More Examples

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)

## Tests

```bash
cargo test -p queue-ext
```

41 tests covering Sink/Stream impl, SendError, drop semantics, and Send/Sync safety.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
