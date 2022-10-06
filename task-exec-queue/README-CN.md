# Task Execution Queue

[English](./README.md)  | 简体中文

*Task Execution Queue* 一个任务执行队列。可限制任务并发执行数量，可控制同一类任务执行顺序。通常，异步任务直接使用Tokio或async-std执行即可；
但是，某些特殊业务场景我们需要批量执行任务，又要控制任务的并发数量时，直接使用spawn()容易导致负载过大，CPU或内存等资源耗尽；此Crate就是为了解决此 类问题而开发的。

## 功能特色

- 可执行任务;
- 可执行任务并返回结果;
- 可限制并发执行任务数量;
- 任务队列;
- 同一类任务顺序执行;
- 支持Local任务

## 计划

## 例子

- quick start

```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_exec_queue::{init_default, default, SpawnDefaultExt};

    let task_runner = init_default();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task ...
        let _ = async {
            println!("hello world!");
        }.spawn().await;

        default().flush().await;
    };
    async_std::task::block_on(root_fut);
}

```

- execute and return result

```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_exec_queue::{Builder, SpawnExt};
    let (exec, task_runner) = Builder::default().workers(10).queue_max(100).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task and return result...
        let res = async {
            "hello world!"
        }.spawn(&exec).result().await;
        println!("result: {:?}", res.ok());

        exec.flush().await;
    };
    async_std::task::block_on(root_fut);
}

```

- sequential execution

```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_exec_queue::{Builder, SpawnExt};

    let (exec, task_runner) =
        Builder::default().workers(10).queue_max(100).group().build::<&str>();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task ...
        let _res = async move {
            println!("hello world!");
        }.spawn(&exec).group("g1").await;

        let res = async move {
            "hello world!"
        }.spawn(&exec).group("g1").result().await;
        println!("result: {:?}", res.ok());

        exec.flush().await;
        println!("exec.actives: {}, waitings: {}, completeds: {}", exec.active_count(), exec.waiting_count(), exec.completed_count());
    };
    async_std::task::block_on(root_fut);
}

```

### 更多例子

- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
