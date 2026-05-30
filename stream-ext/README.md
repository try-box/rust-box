# stream-ext

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*stream-ext* provides an extension trait for Streams that offers rate-limiting combinator functions, allowing you to control the throughput of async streams.

## Features

- `Limiter` trait for custom rate-limiting logic
- `LimiterExt` combinator to wrap any stream with a rate limiter
- Built-in `NoLimiter` for passthrough behavior
- Optional `leaky-bucket` feature for token-bucket rate limiting
- Optional `governor` feature for Governor-based rate limiting

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| *(default)* | Core `Limiter` trait and `NoLimiter` | **yes** |
| `leaky-bucket` | Enable token-bucket rate limiting via `leaky-bucket` crate | no |
| `governor` | Enable rate limiting via `governor` crate | no |

## Basic Usage

```rust
use stream_ext::{IntoLimiter, NoLimiter};
use futures::StreamExt;

let stream = futures::stream::iter(vec![1, 2, 3]);
let limiter = NoLimiter;
let mut limited = stream.limiter(limiter);

// Items pass through without rate limiting
while let Some(item) = limited.next().await {
    println!("{}", item);
}
```

## More Examples

- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)

## Tests

```bash
cargo test -p stream-ext --all-features
```

6 tests covering basic limiting, trait objects, leaky-bucket and governor integrations.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
