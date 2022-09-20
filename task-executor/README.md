# Task Executor

[![GitHub Release](https://img.shields.io/github/release/tryor/task-executor?color=brightgreen)](https://github.com/tryor/task-executor/releases)

English | [简体中文](./README-CN.md)

*Task Executor* Task executors that control the number of parallel executions. Usually, ordinary asynchronous tasks can be 
executed directly using Tokio or async-std; however, in some special business scenarios, we need to perform a certain type 
of tasks in batches, and control the number of concurrent tasks of this type. Using spawn() directly can easily lead to 
excessive load and exhaustion of resources such as CPU or memory. this executor was developed to solve such problems.


## Features

- Execute the tasks;
- Execute the tasks and return results;
- Control the number of concurrently executed tasks;
- Support task queue;

## Plan

- Tasks of the same type can be forced to be executed sequentially;

## Examples

- quick start
```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_executor::{init_default, default, SpawnDefaultExt};

    let task_runner = init_default();
    let global = async move{
        spawn(async {
            //start executor
            task_runner.await;
        });
        //execute future ...
        let _ = async {
            println!("hello world!");
        }.spawn().await;

        default().flush().await;
    };
    async_std::task::block_on(global);
}

```

- execute and return result
```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_executor::{Builder, SpawnExt};
    let (exec, task_runner) = Builder::default().workers(10).queue_max(100).build();
    let global = async move{
        spawn(async {
            //start executor
            task_runner.await;
        });
        //execute future and return result...
        let res = async {
            "hello world!"
        }.spawn(&exec).result().await;
        println!("return result: {:?}", res.ok());

        exec.flush().await;
    };
    async_std::task::block_on(global);
}

```

### More Examples

- [task-executor-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-executor-test.rs)
