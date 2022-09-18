# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

[English](./README.md)  | 简体中文

*Rust Box* 这个 *Crate* 提供了一组用于Rust编程的奇葩工具：

## Crates

The main `rust-box` crate just [re-exports](src/lib.rs) tools from smaller subcrates:

* [`stream-ext`](stream-ext)
  Streams 的扩展特征，提供各种方便的组合函数。
* [`queue-ext`](queue-ext)
  队列的扩展特征，可将队列的pop()函数转换为Stream，push()函数转换为Sender。
* [`tokio-executor`](tokio-executor)
  一个可控制并发执行任务数量的任务执行器。

## 使用

加入下面依赖到你的 `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.2"
```

## 例子

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [tokio-executor-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/tokio-executor-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.