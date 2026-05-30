# box-counter

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*box-counter* 提供并发计数器，支持可选的速率统计。同时支持多线程（基于 `Arc` 的 `Counter`）和单线程（基于 `Rc` 的 `LocalCounter`）变体。

## 功能特色

- **Count 模式** — 跟踪当前值和历史最大值
- **Rate 模式** — 跟踪总数和每秒吞吐率
- 线程安全的 `Counter`（Arc 实现）和轻量级 `LocalCounter`（Rc 实现）
- 通过 `postcard` 支持序列化/反序列化
- 自动更新的速率计算

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `count` | 启用当前值和最大值跟踪 | 否 |
| `rate` | 启用吞吐率跟踪 | 否 |

至少需要启用一个 feature。启用两者可获得完整功能。

## 基本用法

```rust
use counter::Counter;

let c = Counter::new();         // 需要 "count" feature
c.inc();
c.incs(10);
c.dec();
assert_eq!(c.count(), 10);
assert_eq!(c.max(), 10);
```

## 速率模式

```rust
use counter::Counter;
use std::time::Duration;

let c = Counter::new(Duration::from_secs(1)); // 需要 "rate" feature
c.incs(100);
std::thread::sleep(Duration::from_millis(100));
println!("rate: {:.2}/s", c.rate());
```

## 测试

```bash
cargo test -p box-counter --features "count"
cargo test -p box-counter --features "rate"
cargo test -p box-counter --features "count,rate"
```

51 个测试，覆盖 count、rate、组合功能和序列化 roundtrip。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
