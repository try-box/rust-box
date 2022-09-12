# Try Box

[![GitHub Release](https://img.shields.io/github/release/try-box/try-box?color=brightgreen)](https://github.com/try-box/try-box/releases)

[English](./README.md)  | 简体中文

*Try Box* 这个 *Crate* 提供了一组用于Rust编程的奇葩工具：

## Crates
The main `try-box` crate just [re-exports](src/lib.rs) tools from
smaller subcrates:

* [`stream-ext`](stream-ext)
  Streams 的扩展特征，提供各种方便的组合函数。
* [`queue-ext`](queue-ext)
  队列的扩展特征，可将队列的pop()函数转换为Stream，push()函数转换为Sender。

## 使用

加入下面依赖到你的 `Cargo.toml`:

```toml
[dependencies]
try-box = "0.1"
```

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.