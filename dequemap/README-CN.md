# dequemap

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*dequemap* 提供具有 Map 功能的双端队列。它将 `VecDeque` 索引与 `BTreeMap` 或 `HashMap` 后端结合，允许从两端 push/pop 同时保持键值访问。

支持 `no_std` 环境（需要 `alloc`）。

## 功能特色

- **DequeBTreeMap** — 有序映射 + 双端队列操作
- **DequeHashMap** — 哈希映射 + 双端队列操作
- Entry API（VacantEntry / OccupiedEntry），类似 std 的 map entry
- 支持 `no_std`
- 可选的 serde 支持
- Push front/back、pop front/back、remove、retain、clear

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `std` | 使用 Rust 标准库 | **是** |
| `btreemap` | 启用 `DequeBTreeMap`（有序后端） | **是** |
| `hashmap` | 启用 `DequeHashMap`（哈希后端，引入 `hashbrown`） | 否 |
| `serde` | 启用序列化/反序列化支持 | 否 |

## 基本用法

```rust
use dequemap::DequeBTreeMap;

let mut map = DequeBTreeMap::new();
map.push_back("a", 1);
map.push_back("b", 2);
map.push_front("c", 3);

assert_eq!(map.front(), Some((&"c", &3)));
assert_eq!(map.back(), Some((&"b", &2)));
assert_eq!(map.pop_front(), Some(("c", 3)));
```

## 测试

```bash
cargo test -p dequemap --features "btreemap,hashmap,serde"
```

30 个测试，覆盖两个后端、Entry API、serde roundtrip、大量条目和边界情况。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
