# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

[English](./README.md)  | 简体中文

*Rust Box* 提供了一组用于 Rust 编程的实用工具。

## 子项目

| Crate | 描述 | 测试数 |
|---|---|---|
| [`queue-ext`](./queue-ext/) | 队列扩展：将 `pop()` 转为 Stream，`push()` 转为 Sender | 41 |
| [`stream-ext`](./stream-ext/) | Stream 扩展特征，支持速率限制 | 6 |
| [`task-exec-queue`](./task-exec-queue/) | 异步任务执行队列，支持并发控制 | 53 |
| [`event`](./event/) | 事件监听/通知链 | 7 |
| [`std-ext`](./std-ext/) | 标准库功能扩展 | 62 |
| [`mpsc`](./mpsc/) | 基于 queue-ext 的 MPSC 信道 | 37 |
| [`dequemap`](./dequemap/) | 带 Map 功能的双端队列，支持 no_std | 30 |
| [`collections`](./collections/) | BinaryHeap 和 PriorityQueue 封装，支持 no_std | 18 |
| [`counter`](./counter/) | 并发计数器，支持速率统计 | 51 |
| [`convert`](./convert/) | 类型转换工具（字节大小解析） | 8 |
| [`handy-grpc`](./handy-grpc/) | gRPC 客户端/服务器，支持分块传输 | 61 |

## 使用

加入下面依赖到你的 `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.16"
```

或单独使用子 crate：

```toml
[dependencies]
queue-ext = "0.4"
task-exec-queue = "0.10"
```

## Cargo Feature Flags

根 crate 使用 feature 标志来启用各个子 crate 或特定子功能。

### 快速启用

一键启用全部：

```toml
[dependencies]
rust-box = { version = "0.16", features = ["full"] }
```

### Feature 清单

| Feature | 启用内容 | 默认 |
|---|---|---|
| `full` | 启用以下所有 crate 和功能 | 否 |
| **单个子 crate** | | |
| `queue-ext` | `queue-ext` crate | 否 |
| `stream-ext` | `stream-ext` crate | 否 |
| `task-exec-queue` | `task-exec-queue` crate | 否 |
| `event` | `event-notify` crate | 否 |
| `std-ext` | `std-ext` crate | 否 |
| `mpsc` | `mpsc` crate | 否 |
| `dequemap` | `dequemap` crate | 否 |
| `handy-grpc` | `handy-grpc` crate | 否 |
| `collections` | `box-collections` crate | 否 |
| `counter` | `box-counter` crate | 否 |
| `convert` | `box-convert` crate | 否 |
| **子功能** | | |
| `stream-ext-leaky-bucket` | `stream-ext/leaky-bucket` | 否 |
| `stream-ext-governor` | `stream-ext/governor` | 否 |
| `task-exec-queue-rate` | `task-exec-queue/rate` | 否 |
| `mpsc-segqueue` | `mpsc/segqueue` | **是**（`mpsc` 默认） |
| `mpsc-vecdeque` | `mpsc/vecdeque` | 否 |
| `mpsc-priority` | `mpsc/priority` | 否 |
| `mpsc-indexmap` | `mpsc/indexmap` | 否 |
| `dequemap-std` | `dequemap/std` | **是**（`dequemap` 默认） |
| `dequemap-serde` | `dequemap/serde` | 否 |
| `dequemap-btreemap` | `dequemap/btreemap` | **是**（`dequemap` 默认） |
| `dequemap-hashmap` | `dequemap/hashmap` | 否 |
| `collections-std` | `collections/std` | **是**（`collections` 默认） |
| `collections-serde` | `collections/serde` | 否 |
| `collections-binary-heap` | `collections/binary-heap` | 否 |
| `collections-priority-queue` | `collections/priority-queue` | 否 |
| `handy-grpc-reuse` | `handy-grpc/reuse` | 否 |
| `counter-count` | `counter/count` | 否 |
| `counter-rate` | `counter/rate` | 否 |
| `convert-bytesize` | `convert/bytesize` | 否 |

## 测试状态

所有子项目包含全面的单元测试：

```bash
cargo test --workspace --all-features
```

总计：**340+ 个测试**，全部通过。

## 例子

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
- [event-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/event-test.rs)
- [std-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/std-ext-test.rs)
- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)
- [dequemap-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/dequemap-test.rs)
- [counter-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/counter-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
