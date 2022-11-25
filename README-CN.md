# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

[English](./README.md)  | 简体中文

*Rust Box* 这个 *Crate* 提供了一组用于Rust编程的奇葩工具：

## Crates

The main `rust-box` crate just [re-exports](src/lib.rs) tools from smaller subcrates:

* [`stream-ext`](https://github.com/try-box/rust-box/tree/main/stream-ext)
  Streams 的扩展特征，提供各种方便的组合函数。
* [`queue-ext`](https://github.com/try-box/rust-box/tree/main/queue-ext)
  队列的扩展特征，可将队列的pop()函数转换为Stream，push()函数转换为Sender。
* [`task-exec-queue`](https://github.com/try-box/rust-box/tree/main/task-exec-queue)
  一个任务执行队列。可限制任务并发执行数量，可控制同一类任务执行顺序。
* [`event`](https://github.com/try-box/rust-box/tree/main/event)
  事件
* [`std-ext`](https://github.com/try-box/rust-box/tree/main/std-ext)
  扩展标准库功能，使标准库用起来更方便
* [`mpsc`](https://github.com/try-box/rust-box/tree/main/mpsc)
  基于queue-ext实现的MPSC信道

## 使用

加入下面依赖到你的 `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.6"
```

## 例子

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
- [event-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/event-test.rs)
- [std-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/std-ext-test.rs)
- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
