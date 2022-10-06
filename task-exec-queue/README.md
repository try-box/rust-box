# Task Execution Queue

English | [简体中文](./README-CN.md)

*Task Execution Queue* A task execution queue. Can limit the number of concurrent tasks and execution order of the same
type of tasks can be controlled. Generally, asynchronous tasks can be executed directly using Tokio or async-std;
However, in some special business scenarios, when we need to execute tasks in batches and control the concurrent number
of tasks, using spawn() directly can easily lead to excessive load and exhaustion of resources such as CPU or memory;
This Crate is developed to solve such problems.

## Features

- Execute the tasks;
- Execute the tasks and return results;
- Limit the number of concurrent tasks
- Task queue;
- Sequential execution of the same type of tasks;
- Support Local tasks

## Plan

## Examples

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

### More Examples

- [task-exec-queue-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-exec-queue-test.rs)
