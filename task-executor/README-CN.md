# Task Executor

[English](./README.md)  | 简体中文

*Task Executor* 可控制并行执行数量的任务执行器。通常，普通异步任务直接使用Tokio或async-std执行即可；但是，某些特殊业务场景我们需要批量执行某一类型任务， 又要控制此类任务的并发数量时，直接使用spawn()
容易导致负载过大，CPU或内存等资源耗尽；此执行器就是为了解决此类问题而开发的。

## 功能特色

- 执行任务;
- 执行任务并返回结果;
- 可控制并行执行任务上限数量;
- 支持任务队列;
- 同一类任务顺序执行;

## 计划


## 例子

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

- sequential execution

```rust
fn main() {
    use async_std::task::spawn;
    use rust_box::task_executor::{Builder, SpawnExt};

    let (exec, task_runner) =
        Builder::default().workers(10).queue_max(100).group().build::<&str>();
    
    let global = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute future ...
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
    async_std::task::block_on(global);
}

```

### 更多例子

- [task-executor-test.rs](https://github.com/try-box/rust-box/blob/main/examples/src/task-executor-test.rs)
