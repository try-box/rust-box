# event-notify

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*event-notify* 提供轻量级的事件监听/通知链实现。创建带链式监听器的事件，每个监听器可以处理或转发事件。

无需外部依赖——纯 Rust 标准库实现。

## 功能特色

- 轻量级事件监听模式
- 通过 `Next` 转发的链式监听器
- 支持任意参数和返回类型的事件触发
- 零依赖

## Cargo Feature Flags

此 crate 没有 Cargo feature 标志，所有功能默认启用。

## 基本用法

```rust
use event_notify::Event;

let event = Event::<i32, i32>::listen(|args, _next| {
    args * 2
}).finish();

let result = event.fire(21);
assert_eq!(result, 42);
```

## 测试

```bash
cargo test -p event-notify
```

7 个测试，覆盖单监听器、链式监听器、链中断和多次触发场景。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
