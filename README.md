# Rust Box

<a href="https://github.com/try-box/rust-box/releases"><img alt="GitHub Release" src="https://img.shields.io/github/release/try-box/rust-box?color=brightgreen" /></a>
<a href="https://crates.io/crates/rust-box"><img alt="crates.io" src="https://img.shields.io/crates/v/rust-box" /></a>
<a href="https://docs.rs/rust-box"><img alt="Documentation" src="https://docs.rs/rust-box/badge.svg" /></a>

English | [简体中文](./README-CN.md)

*Rust Box* provides an odd set of tools for Rust programming.

## Crates

The main `rust-box` crate re-exports tools from smaller subcrates:

| Crate | Description | Tests |
|---|---|---|
| [`queue-ext`](./queue-ext/) | Extended queue: convert `pop()` to Stream, `push()` to Sender | 41 |
| [`stream-ext`](./stream-ext/) | Stream extension trait with rate-limiting combinators | 6 |
| [`task-exec-queue`](./task-exec-queue/) | Async task execution queue with concurrency control | 53 |
| [`event`](./event/) | Event listener/notification chain | 7 |
| [`std-ext`](./std-ext/) | Standard library extensions | 62 |
| [`mpsc`](./mpsc/) | MPSC channel based on queue-ext | 37 |
| [`dequemap`](./dequemap/) | Double-ended queue with map features, no_std | 30 |
| [`collections`](./collections/) | BinaryHeap and PriorityQueue wrappers, no_std | 18 |
| [`counter`](./counter/) | Concurrent counter with rate tracking | 51 |
| [`convert`](./convert/) | Type conversion utilities (byte size parsing) | 8 |
| [`handy-grpc`](./handy-grpc/) | gRPC client/server with chunked transfer | 61 |

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust-box = "0.16"
```

Or use individual crates:

```toml
[dependencies]
queue-ext = "0.4"
task-exec-queue = "0.10"
```

## Cargo Feature Flags

The root crate uses feature flags to enable individual subcrates or specific sub-features.

### Quick enable

Enable everything at once:

```toml
[dependencies]
rust-box = { version = "0.16", features = ["full"] }
```

### Feature table

| Feature | Enables | Default |
|---|---|---|
| `full` | All crates and features below | no |
| **Individual crates** | | |
| `queue-ext` | `queue-ext` crate | no |
| `stream-ext` | `stream-ext` crate | no |
| `task-exec-queue` | `task-exec-queue` crate | no |
| `event` | `event-notify` crate | no |
| `std-ext` | `std-ext` crate | no |
| `mpsc` | `mpsc` crate | no |
| `dequemap` | `dequemap` crate | no |
| `handy-grpc` | `handy-grpc` crate | no |
| `collections` | `box-collections` crate | no |
| `counter` | `box-counter` crate | no |
| `convert` | `box-convert` crate | no |
| **Sub-features** | | |
| `stream-ext-leaky-bucket` | `stream-ext/leaky-bucket` | no |
| `stream-ext-governor` | `stream-ext/governor` | no |
| `task-exec-queue-rate` | `task-exec-queue/rate` | no |
| `mpsc-segqueue` | `mpsc/segqueue` | **yes** (via `mpsc` default) |
| `mpsc-vecdeque` | `mpsc/vecdeque` | no |
| `mpsc-priority` | `mpsc/priority` | no |
| `mpsc-indexmap` | `mpsc/indexmap` | no |
| `dequemap-std` | `dequemap/std` | **yes** (via `dequemap` default) |
| `dequemap-serde` | `dequemap/serde` | no |
| `dequemap-btreemap` | `dequemap/btreemap` | **yes** (via `dequemap` default) |
| `dequemap-hashmap` | `dequemap/hashmap` | no |
| `collections-std` | `collections/std` | **yes** (via `collections` default) |
| `collections-serde` | `collections/serde` | no |
| `collections-binary-heap` | `collections/binary-heap` | no |
| `collections-priority-queue` | `collections/priority-queue` | no |
| `handy-grpc-reuse` | `handy-grpc/reuse` | no |
| `counter-count` | `counter/count` | no |
| `counter-rate` | `counter/rate` | no |
| `convert-bytesize` | `convert/bytesize` | no |

## Test Status

All crates include comprehensive unit tests. Run with:

```bash
cargo test --workspace --all-features
```

Total: **~340+ tests**, all passing.

## Examples

- [queue-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/queue-ext-test.rs)
- [stream-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/stream-ext-test.rs)
- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
- [event-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/event-test.rs)
- [std-ext-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/std-ext-test.rs)
- [channel-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/channel-test.rs)
- [dequemap-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/dequemap-test.rs)
- [counter-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/counter-test.rs)

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
