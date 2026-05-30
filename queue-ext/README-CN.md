# queue-ext

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*queue-ext* 提供队列的扩展功能，可将队列的 `pop()` 函数转换为 Stream，`push()` 函数转换为 Sender。是异步信道实现的基础组件。

## 功能特色

- 将队列 `pop()` 转换为 `Stream` 实现
- 将队列 `push()` 转换为 `Sink`/`Sender` 实现
- 通过 `queue_channel()` 提供类型擦除的信道 API
- `QueueExt` 特征，便于与任意队列类型集成
- `SendError` 类型，支持 `full` 和 `disconnected` 两种变体
- 支持任意队列后端（SegQueue、VecDeque 等）
- 线程安全（`Send` + `Sync`）

## Cargo Feature Flags

此 crate 没有 Cargo feature 标志，所有功能默认启用。

## 基本用法

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
```

## 更多例子

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)

## 测试

```bash
cargo test -p queue-ext
```

41 个测试，覆盖 Sink/Stream 实现、SendError、drop 语义、Send/Sync 安全性。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
