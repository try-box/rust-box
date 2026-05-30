# stream-ext

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*stream-ext* 为 Stream 提供扩展特征，支持速率限制的组合函数，可用于控制异步流的吞吐量。

## 功能特色

- `Limiter` 特征，支持自定义速率限制逻辑
- `LimiterExt` 组合子，为任意 Stream 包裹速率限制器
- 内置 `NoLimiter` 透传实现
- 可选 `leaky-bucket` 功能，支持令牌桶速率限制
- 可选 `governor` 功能，支持 Governor 速率限制

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| *(默认)* | 核心 `Limiter` 特征和 `NoLimiter` | **是** |
| `leaky-bucket` | 通过 `leaky-bucket` crate 启用令牌桶速率限制 | 否 |
| `governor` | 通过 `governor` crate 启用速率限制 | 否 |

## 基本用法

```rust
use stream_ext::{IntoLimiter, NoLimiter};
use futures::StreamExt;

let stream = futures::stream::iter(vec![1, 2, 3]);
let limiter = NoLimiter;
let mut limited = stream.limiter(limiter);
```

## 更多例子

- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)

## 测试

```bash
cargo test -p stream-ext --all-features
```

6 个测试，覆盖基本限制、trait 对象、leaky-bucket 和 governor 集成。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
