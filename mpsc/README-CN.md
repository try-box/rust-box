# mpsc

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*mpsc* 基于 `queue-ext` 实现的多生产者单消费者信道。支持多种队列后端实现，各有不同特性。

## 功能特色

- **SegQueue** 信道 — 无锁，基于 crossbeam（默认）
- **VecDeque** 信道 — 标准 VecDeque 实现
- **Priority** 信道 — 按键顺序优先级排列
- **IndexMap** 信道 — 按键去重（后写覆盖）
- 线程安全的 `Sender`/`Receiver` 类型
- 单线程 `LocalSender`/`LocalReceiver` 变体
- `Sink` 和 `Stream` 特征实现
- 类型擦除的信道 API

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `segqueue` | 基于 `crossbeam::SegQueue` 的无锁信道 | **是** |
| `vecdeque` | 基于 `std::collections::VecDeque` 的信道 | 否 |
| `priority` | 基于 `box-collections::PriorityQueue` 的优先级信道 | 否 |
| `indexmap` | 基于 `indexmap::IndexMap` 的去重信道 | 否 |

## 基本用法

```rust
use mpsc::segqueue_channel;
use futures::executor::block_on;

let (mut tx, mut rx) = segqueue_channel(10);
block_on(tx.send(42)).unwrap();
assert_eq!(block_on(rx.recv()), Some(42));
```

## 更多例子

- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)

## 测试

```bash
cargo test -p mpsc --features "segqueue,vecdeque,priority,indexmap"
```

37 个测试，覆盖全部 4 种信道类型、LocalSender/Receiver、克隆和 drop 语义。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
