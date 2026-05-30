# std-ext

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*std-ext* 扩展 Rust 标准库，提供常用模式的附加特征和工具，使标准类型使用起来更方便。

## 功能特色

- `ArcExt` 特征 — 通过 `.arc()` 将任意值包装到 Arc 中
- `AtomicExt` 特征 — 将基本类型转换为原子类型
- `CacheMapExt` — 支持 TTL 的缓存实现（HashMap、BTreeMap、DequeBTreeMap、DequeHashMap）
- `EntryExt` — 支持 TTL 的扩展 Entry API
- `TimedValue` — 带过期时间的值包装器
- `OrdExt` / `HashExt` / `OrdHashExt` — 排序和哈希包装器
- 元组辅助宏：`tuple_deref!`、`tuple_deref_mut!`、`tuple_take!`

## Cargo Feature Flags

此 crate 没有 Cargo feature 标志，所有功能默认启用。

## 基本用法

```rust
use std_ext::{ArcExt, AtomicExt};
use std::sync::atomic::Ordering;

let v = 42.arc();
assert_eq!(*v, 42);

let a = 0usize.atomic();
a.store(10, Ordering::SeqCst);
assert_eq!(a.load(Ordering::SeqCst), 10);
```

## 测试

```bash
cargo test -p std-ext --all-features
```

62 个测试，覆盖 ArcExt、AtomicExt、CacheMapExt、EntryExt、TimedValue 和包装器类型。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
