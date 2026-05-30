# handy-grpc

<!-- Badges placeholder -->

English | [简体中文](./README-CN.md)

*handy-grpc* provides a convenient gRPC client/server implementation built on top of `tonic`. Features include chunked message transfer, mailbox-based request/response handling, TLS support, and authentication.

## Features

- gRPC client with connection management and reconnect
- gRPC server with TLS and token authentication
- Chunked message transfer for large payloads
- `Mailbox` / `DuplexMailbox` for request-response patterns
- Priority-based message sending
- Optional rate tracking
- Optional socket reuse (reuseport, reuseaddr)

## Cargo Feature Flags

| Feature | Description | Default |
|---|---|---|
| `rate` | Enable rate tracking via `box-counter` | no |
| `rate_print` | Enable `rate` and periodic rate logging | no |
| `reuse` | Enable both `reuseport` and `reuseaddr` | no |
| `reuseport` | Enable `SO_REUSEPORT` socket option (requires `socket2`) | no |
| `reuseaddr` | Enable `SO_REUSEADDR` socket option (requires `socket2`) | no |

## Basic Usage

```rust
use handy_grpc::ClientBuilder;

let client = ClientBuilder::new()
    .connect("http://127.0.0.1:50051")
    .await
    .unwrap();
```

## Tests

```bash
cargo test -p handy-grpc
```

61 tests covering ChunkedBuffer, Mailbox/DuplexMailbox, Client/Server builders, and serialization.

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
