# box-counter

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*box-counter* provides a concurrent counter with optional rate tracking. Supports both multi-threaded (`Counter` via `Arc`) and single-threaded (`LocalCounter` via `Rc`) variants.

## Features

- **Count mode** — track current value and maximum reached
- **Rate mode** — track total count and per-second throughput rate
- Thread-safe `Counter` (Arc-based) and lightweight `LocalCounter` (Rc-based)
- Serialize/Deserialize support via `postcard`
- Auto-updating rate calculation

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `count` | Enable current value and max tracking | no |
| `rate` | Enable throughput rate tracking | no |

At least one feature must be enabled. Enable both for full functionality.

## Basic Usage

```rust
use counter::Counter;

let c = Counter::new();         // requires "count" feature
c.inc();
c.incs(10);
c.dec();
assert_eq!(c.count(), 10);
assert_eq!(c.max(), 10);
```

## Rate Mode

```rust
use counter::Counter;
use std::time::Duration;

let c = Counter::new(Duration::from_secs(1)); // requires "rate" feature
c.incs(100);
std::thread::sleep(Duration::from_millis(100));
println!("rate: {:.2}/s", c.rate());
```

## Tests

```bash
cargo test -p box-counter --features "count"
cargo test -p box-counter --features "rate"
cargo test -p box-counter --features "count,rate"
```

51 tests covering count, rate, combined features, and serialization roundtrip.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
