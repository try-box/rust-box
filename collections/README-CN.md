# box-collections

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*box-collections* 提供 `no_std` 兼容的 `alloc::collections::BinaryHeap` 封装，实现了带扩展功能的 `BinaryHeap` 和 `PriorityQueue`，支持排序迭代、drain 排序和可选的 serde 支持。

## 功能特色

- **BinaryHeap** — 优先级队列，支持排序迭代和 drain
- **PriorityQueue** — 优先级-值对，支持自定义排序
- 支持 `no_std`（需要 `alloc`）
- 可选的 serde 支持
- `iter_sorted()`、`drain_sorted()`、`into_sorted_vec()` 方法

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `std` | 使用 Rust 标准库 | **是** |
| `binary-heap` | 启用 `BinaryHeap` 封装 | 否 |
| `priority-queue` | 启用 `PriorityQueue` 封装 | 否 |
| `serde` | 启用序列化/反序列化支持 | 否 |

## 基本用法

```rust
use collections::PriorityQueue;

let mut pq = PriorityQueue::new();
pq.push(3, "low");
pq.push(1, "high");
pq.push(2, "mid");

assert_eq!(pq.pop(), Some((1, "high")));
```

## 测试

```bash
cargo test -p box-collections --features "binary-heap,priority-queue,serde"
```

18 个测试，覆盖空、单元素、重复元素、排序迭代和 serde roundtrip。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
