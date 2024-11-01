# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

English | [简体中文](./README-CN.md)

*Rust Box* This crate provides an odd set of tools for Rust programming:

## Crates

The main `rust-box` crate just [re-exports](src/lib.rs) tools from smaller subcrates:

* [`stream-ext`](https://github.com/try-box/rust-box/tree/main/stream-ext)
  An extension trait for Streams that provides a variety of convenient combinator functions.
* [`queue-ext`](https://github.com/try-box/rust-box/tree/main/queue-ext)
  The extended feature of the queue can convert the pop() function of the queue to Stream and the push() function to
  Sender.
* [`task-exec-queue`](https://github.com/try-box/rust-box/tree/main/task-exec-queue)
  A task execution queue. Can limit the number of concurrent tasks and execution order of the same type of tasks can be
  controlled.
* [`event`](https://github.com/try-box/rust-box/tree/main/event)
  Event
* [`std-ext`](https://github.com/try-box/rust-box/tree/main/std-ext)
  Expand the function of the standard library to make it more convenient to use
* [`mpsc`](https://github.com/try-box/rust-box/tree/main/mpsc)
  MPSC channel based on queue-ext
* [`dequemap`](https://github.com/try-box/rust-box/tree/main/dequemap)
  Double-ended queue with Map feature


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.12"
```

## Examples

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
- [event-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/event-test.rs)
- [std-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/std-ext-test.rs)
- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)
- [dequemap-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/dequemap-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
