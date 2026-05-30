# event-notify

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*event-notify* provides a lightweight event listener/notification chain implementation. Create events with chained listeners where each listener can process or forward the event.

No external dependencies required — pure Rust standard library.

## Features

- Lightweight event listener pattern
- Chained listeners via `Next` forwarding
- Fire events with arbitrary payload and return types
- Zero dependencies

## Cargo Feature Flags

This crate has no Cargo feature flags. All functionality is enabled by default.

## Basic Usage

```rust
use event_notify::Event;

let event = Event::<i32, i32>::listen(|args, _next| {
    args * 2
}).finish();

let result = event.fire(21);
assert_eq!(result, 42);
```

## Tests

```bash
cargo test -p event-notify
```

7 tests covering single listener, chained listeners, chain break, and multiple fire scenarios.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
