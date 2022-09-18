# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

English | [简体中文](./README-CN.md)

*Rust Box* This crate provides an odd set of tools for Rust programming:

## Crates

The main `rust-box` crate just [re-exports](src/lib.rs) tools from smaller subcrates:

* [`stream-ext`](stream-ext)
  An extension trait for Streams that provides a variety of convenient combinator functions.
* [`queue-ext`](queue-ext)
  The extended feature of the queue can convert the pop() function of the queue to Stream and the push() function to
  Sender.
* [`tokio-executor`](tokio-executor)
  A task executor that controls the number of concurrently executing tasks.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.2"
```

## Examples

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [tokio-executor-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/tokio-executor-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.