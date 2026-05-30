# handy-grpc

<!-- Badges placeholder -->

[English](./README.md)  | 简体中文

*handy-grpc* 基于 `tonic` 提供便捷的 gRPC 客户端/服务器实现。特性包括分块消息传输、基于 mailbox 的请求/响应处理、TLS 支持和认证。

## 功能特色

- gRPC 客户端，支持连接管理和重连
- gRPC 服务器，支持 TLS 和 Token 认证
- 大消息分块传输
- `Mailbox` / `DuplexMailbox` 请求-响应模式
- 基于优先级的消息发送
- 可选的速率统计
- 可选的 socket 重用（reuseport、reuseaddr）

## Cargo Feature Flags

| Feature | 说明 | 默认 |
|---|---|---|
| `rate` | 通过 `box-counter` 启用速率统计 | 否 |
| `rate_print` | 启用 `rate` 和定期速率日志输出 | 否 |
| `reuse` | 同时启用 `reuseport` 和 `reuseaddr` | 否 |
| `reuseport` | 启用 `SO_REUSEPORT` socket 选项（需要 `socket2`） | 否 |
| `reuseaddr` | 启用 `SO_REUSEADDR` socket 选项（需要 `socket2`） | 否 |

## 基本用法

```rust
use handy_grpc::ClientBuilder;

let client = ClientBuilder::new()
    .connect("http://127.0.0.1:50051")
    .await
    .unwrap();
```

## 测试

```bash
cargo test -p handy-grpc
```

61 个测试，覆盖 ChunkedBuffer、Mailbox/DuplexMailbox、Client/Server 构建器和序列化。

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
