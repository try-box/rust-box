# box-convert

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*box-convert* 提供类型转换工具，当前包含 `Bytesize` 类型，用于解析人类可读的字节大小字符串（如 "1K2M3B"）并将字节数格式化为可读字符串。

## 功能特色

- **Bytesize** — 人类可读的字节大小解析和格式化
- 支持 K/M/G 后缀（如 "1K"、"2M"、"3G"）
- 组合格式解析（如 "1K2M3B"）
- Serde 支持，可用于 JSON 序列化
- `From<usize>` 和 `From<&str>` 转换

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `bytesize` | 启用 `Bytesize` 类型，支持人类可读的字节大小解析 | 否 |

## 基本用法

```rust
use convert::Bytesize;

let b = Bytesize::from("1K");
assert_eq!(*b, 1024);

let b = Bytesize::from(2048usize);
assert_eq!(b.string(), "2K");
```

## 测试

```bash
cargo test -p box-convert --features "bytesize"
```

8 个测试，覆盖 From 实现、字符串格式化、serde roundtrip 和边界情况。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
