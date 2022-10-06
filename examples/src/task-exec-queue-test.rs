#![allow(unused_must_use)]
#![allow(dead_code)]


use futures::{Sink, Stream};
use crossbeam_queue::SegQueue;
use rust_box::queue_ext::{Action, QueueExt, Reply};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

fn main() {
    std::env::set_var("RUST_LOG", "task_exec_queue_test=info");
    env_logger::init();

    test_quick_start();
    test_return_result();
    test_default_set();
    test_task_exec_queue();

    test_channel();
    test_channel_custom();
    test_channel_with_name();

    test_group();
    test_group_bench();

    test_local();
}


//quick start
fn test_quick_start() {
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

fn test_return_result() {
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

fn test_channel() {
    use futures::channel::mpsc;
    use async_std::task::{sleep, spawn};
    use rust_box::task_exec_queue::{Builder, SpawnExt};
    let queue_max = 100;
    let (tx, rx) =  mpsc::channel(queue_max);
    let (mut exec, task_runner) = Builder::default().workers(10).with_channel(tx, rx).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        let exec1 = exec.clone();
        let exec2 = exec.clone();

        spawn(async move {
            let _res = async {
                println!("with channel: hello world!");
            }.spawn(&exec1).result().await;
        });

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(100)).await;
                println!("with channel and result: hello world!");
                100
            }.spawn(&exec2).result().await;
            println!("result: {:?}", res.ok());
        });

        exec.spawn(async {
            println!("hello world!");
        }).result().await;

        exec.flush().await;
        println!("2 exec.actives: {}, waitings: {}, completeds: {}", exec.active_count(), exec.waiting_count(), exec.completed_count());
    };
    async_std::task::block_on(root_fut);
}


fn test_channel_custom() {
    use async_std::task::spawn;
    use rust_box::task_exec_queue::{Builder, SpawnExt, TaskType};
    let queue_max = 100;
    let (tx, rx) = channel::<TaskType>(queue_max);
    let (exec, task_runner) = Builder::default().workers(10).with_channel(tx, rx).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        let res = async {
            "hello world!"
        }.spawn(&exec).result().await;
        println!("result: {:?}", res.ok());

        exec.flush().await;
    };
    async_std::task::block_on(root_fut);
}

fn channel<'a, T>(cap: usize) -> (impl Sink<((), T)> + Clone, impl Stream<Item=((), T)>)
{
    let (tx, rx) = Arc::new(SegQueue::new()).queue_channel(
        move |s, act| match act {
            Action::Send(val) => {
                s.push(val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.len() >= cap),
            Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
        },
        |s, _| {
            if s.is_empty() {
                Poll::Pending
            } else {
                match s.pop() {
                    Some(m) => Poll::Ready(Some(m)),
                    None => Poll::Pending,
                }
            }
        },
    );
    (tx, rx)
}

fn test_channel_with_name() {
    use async_std::task::{sleep, spawn};
    use rust_box::task_exec_queue::{Builder, SpawnExt};
    let queue_max = 100;
    let (tx, rx) =  channel_with_name(queue_max);
    let (mut exec, task_runner) = Builder::default().workers(10).with_channel(tx, rx).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        let exec1 = exec.clone();
        let exec2 = exec.clone();

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(100)).await;
                "hello world!"
            }.spawn_with(&exec1, "test1").result().await;
            println!("1 with name result: {:?}", res.ok());
        });

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(100)).await;
                "hello world!"
            }.spawn_with(&exec2, "test1").result().await;
            println!("2 with name result: {:?}", res.ok());
        });

        exec.spawn_with(async {
            println!("hello world!");
        }, "test1").await;

        // exec.flush().await;
        sleep(Duration::from_millis(100)).await;
        println!("exec.actives: {}, waitings: {}, completeds: {}", exec.active_count(), exec.waiting_count(), exec.completed_count());
    };
    async_std::task::block_on(root_fut);
}

fn channel_with_name<'a, T>(cap: usize) -> (impl Sink<(&'a str, T)> + Clone, impl Stream<Item=(&'a str, T)>)
{
    use parking_lot::RwLock;
    use linked_hash_map::LinkedHashMap;
    let (tx, rx) = Arc::new(RwLock::new(LinkedHashMap::new())).queue_channel(
        move |s, act| match act {
            Action::Send((key, val)) => {
                let mut s = s.write();
                if s.contains_key(&key) {
                    println!("remove old, {}", key);
                    s.remove(&key);
                }
                s.insert(key, val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.read().len() >= cap),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
        },
        |s, _| {
            let mut s = s.write();
            if s.is_empty() {
                Poll::Pending
            } else {
                match s.pop_front() {
                    Some(m) => Poll::Ready(Some(m)),
                    None => Poll::Pending,
                }
            }
        },
    );
    (tx, rx)
}

fn test_default_set() {
    use tokio::{task::spawn};
    use rust_box::task_exec_queue::{Builder, set_default, default, SpawnDefaultExt};

    //set default task execution queue
    let (exec, task_runner) = Builder::default().workers(100).queue_max(1000).build();
    set_default(exec);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task ...
        let res = async {
            log::info!("execute task ...");
        }.spawn().await;
        assert_eq!(res.ok(), Some(()));

        //execute task and return result...
        let res = async {
            3 + 2 - 5 + 100
        }.spawn().result().await;
        assert_eq!(res.as_ref().ok(), Some(&100));
        log::info!("execute and result is {:?}", res.ok());

        default().flush().await;
    });
}

fn test_task_exec_queue() {
    use tokio::{task::spawn, time::sleep};
    use rust_box::task_exec_queue::Builder;
    const MAX_TASKS: isize = 100_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = Builder::default().workers(150).queue_max(1000).build();
    let mailbox = exec.clone();
    let root_fut = async move {

        spawn(async move {
            task_runner.await;
        });

        spawn(async move {
            for i in 0..MAX_TASKS {
                let mut mailbox = mailbox.clone();
                spawn(async move {

                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            sleep(std::time::Duration::from_micros(1)).await;
                            i
                        }).await;

                    //send and wait reply
                    let _res = mailbox.spawn(async move {
                        sleep(std::time::Duration::from_micros(1)).await;
                        i * i + 100
                    }).result().await;

                });
            }
        });

        for i in 0..10 {
            log::info!(
                "{}  {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_flushing(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(500)).await;
        }

        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASKS * 2);

        log::info!(
            "close {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count(),
            exec.rate()
        );
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(root_fut);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}


//group
fn test_group() {
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

fn test_group_bench() {
    use tokio::{task::spawn, time::sleep};
    use rust_box::task_exec_queue::Builder;
    const MAX_TASKS: isize = 100_0000;
    let now = std::time::Instant::now();
    let (exec, task_runner) =
        Builder::default().workers(100).queue_max(10000).group().build::<isize>();
    let mailbox = exec.clone();
    let runner = async move {
        spawn(async move {
            task_runner.await;
        });

        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let mut mailbox = mailbox.clone();
                spawn(async move {

                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            // sleep(std::time::Duration::from_nanos(1)).await;
                        }).group(i % 10).await;

                    //send and wait reply
                    let _res = mailbox.spawn(async move {
                        // sleep(std::time::Duration::from_nanos(1)).await;
                        i * i + 100
                    }).group(i % 100).result().await;
                    // log::info!("calc: {} * {} + 100 = {:?}", i, i, _res.ok());
                });
            }
        });

        for i in 0..10 {
            log::info!(
                "{}  {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_flushing(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(500)).await;
        }

        test_spawns.await;
        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASKS * 2);

        log::info!(
            "close {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count(),
            exec.rate()
        );
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}

fn test_local() {
    use tokio::task::spawn_local;
    use rust_box::task_exec_queue::{LocalBuilder, LocalSpawnExt};
    let (exec, task_runner) = LocalBuilder::default().workers(10).queue_max(100).build();
    let root_fut = async move {
        spawn_local(async {
            //start executor
            task_runner.await;
        });

        //execute task and return result...
        let res = async {
            "hello world!"
        }.spawn(&exec).result().await;
        println!("result: {:?}", res.ok());

        exec.flush().await;
        exec.close().await;
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}