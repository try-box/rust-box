#![allow(unused_must_use)]
#![allow(dead_code)]

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use crossbeam_queue::SegQueue;
use futures::{Sink, Stream};
use rust_box::queue_ext::{Action, QueueExt, Reply};

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
    test_local_with_channel();
    test_local_group_with_channel();
    test_local_task_exec_queue();
}

//quick start
fn test_quick_start() {
    use async_std::task::spawn;
    use rust_box::task_exec_queue::{default, init_default, SpawnDefaultExt};

    let task_runner = init_default();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task ...
        let _ = async {
            log::info!("[test_quick_start] hello world!");
        }
        .spawn()
        .await;

        default().flush().await;

        log::info!(
            "[test_quick_start] completed_count: {}, waiting_count: {}, active_count: {}",
            default().completed_count(),
            default().waiting_count(),
            default().active_count()
        );

        assert!(
            default().completed_count() == 1
                && default().waiting_count() == 0
                && default().active_count() == 0
        );
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
        let res = async { "hello world!" }.spawn(&exec).result().await;
        log::info!("[test_return_result] result: {:?}", res.as_ref().ok());

        log::info!(
            "[test_return_result] completed_count: {}, waiting_count: {}, active_count: {}",
            exec.completed_count(),
            exec.waiting_count(),
            exec.active_count(),
        );

        assert!(
            exec.completed_count() == 1
                && exec.waiting_count() == 0
                && exec.active_count() == 0
                && res.is_ok()
        );

        exec.flush().await;
    };
    async_std::task::block_on(root_fut);
}

fn test_channel() {
    use async_std::task::{sleep, spawn};
    use futures::channel::mpsc;
    use rust_box::task_exec_queue::{Builder, SpawnExt};
    let queue_max = 100;
    let (tx, rx) = mpsc::channel(queue_max);
    let (exec, task_runner) = Builder::default().workers(10).with_channel(tx, rx).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        let exec1 = exec.clone();
        let exec2 = exec.clone();

        spawn(async move {
            let _res = async {
                log::info!("[test_channel] with mpsc: hello world!");
            }
            .spawn(&exec1)
            .result()
            .await;
        });

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(100)).await;
                log::info!("[test_channel] with mpsc and result: hello world!");
                100
            }
            .spawn(&exec2)
            .result()
            .await;
            log::info!("[test_channel] result: {:?}", res.ok());
        });

        exec.spawn(async {
            log::info!("[test_channel] hello world!");
        })
        .result()
        .await;

        exec.flush().await;
        log::info!(
            "[test_channel]  exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );

        assert!(
            exec.completed_count() == 3 && exec.waiting_count() == 0 && exec.active_count() == 0
        );
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

        let res = async { "hello world!" }.spawn(&exec).result().await;
        log::info!("[test_channel_custom] result: {:?}", res.ok());

        exec.flush().await;

        log::info!(
            "[test_channel_custom]  exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );

        assert!(
            exec.completed_count() == 1 && exec.waiting_count() == 0 && exec.active_count() == 0
        );
    };
    async_std::task::block_on(root_fut);
}

fn channel<'a, T>(cap: usize) -> (impl Sink<((), T)> + Clone, impl Stream<Item = ((), T)>) {
    let (tx, rx) = Arc::new(SegQueue::new()).queue_channel(
        move |s, act| match act {
            Action::Send(val) => {
                s.push(val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.len() >= cap),
            Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
            Action::Len => Reply::Len(s.len()),
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
    let (tx, rx) = channel_with_name(queue_max);
    let (exec, task_runner) = Builder::default().workers(10).with_channel(tx, rx).build();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        let exec1 = exec.clone();
        let exec2 = exec.clone();

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(50)).await;
                "a hello world!"
            }
            .spawn_with(&exec1, "test1")
            .result()
            .await;
            log::info!(
                "[test_channel_with_name] 1 with name result: {:?}",
                res.ok()
            );
        });

        spawn(async move {
            let res = async {
                sleep(Duration::from_micros(50)).await;
                "b hello world!"
            }
            .spawn_with(&exec2, "test1")
            .result()
            .await;
            log::info!(
                "[test_channel_with_name] 2 with name result: {:?}",
                res.ok()
            );
        });
        log::info!(
            "[test_channel_with_name] 2 exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );
        sleep(Duration::from_micros(70)).await;
        exec.spawn_with(
            async {
                log::info!("[test_channel_with_name] c hello world!");
            },
            "test1",
        )
        .await;
        log::info!(
            "[test_channel_with_name] 3 exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );
        exec.flush().await;
        //        sleep(Duration::from_millis(200)).await;
        log::info!(
            "[test_channel_with_name] 4 exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );

        assert!(
            (exec.completed_count() == 2 || exec.completed_count() == 3)
                && exec.waiting_count() == 0
                && exec.active_count() == 0
        );
    };
    async_std::task::block_on(root_fut);
}

fn channel_with_name<'a, T>(
    cap: usize,
) -> (
    impl Sink<(&'a str, T)> + Clone,
    impl Stream<Item = (&'a str, T)>,
) {
    use linked_hash_map::LinkedHashMap;
    use parking_lot::RwLock;
    let (tx, rx) = Arc::new(RwLock::new(LinkedHashMap::new())).queue_channel(
        move |s, act| match act {
            Action::Send((key, val)) => {
                let mut s = s.write();
                if s.contains_key(&key) {
                    log::info!("[channel_with_name] remove old, {}", key);
                    s.remove(&key);
                }
                s.insert(key, val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.read().len() >= cap),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
            Action::Len => Reply::Len(s.read().len()),
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
    use rust_box::task_exec_queue::{default, set_default, Builder, SpawnDefaultExt};
    use tokio::task::spawn;

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
            log::info!("[test_default_set] execute task ...");
        }
        .spawn()
        .await;
        assert_eq!(res.ok(), Some(()));

        //execute task and return result...
        let res = async { 3 + 2 - 5 + 100 }.spawn().result().await;
        assert_eq!(res.as_ref().ok(), Some(&100));
        log::info!("[test_default_set] execute and result is {:?}", res.ok());

        default().flush().await;
    });
}

fn test_task_exec_queue() {
    use rust_box::task_exec_queue::Builder;
    use tokio::{task::spawn, time::sleep};
    const MAX_TASKS: isize = 30_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = Builder::default().workers(200).queue_max(1000).build();
    let mailbox = exec.clone();
    let root_fut = async move {
        spawn(async move {
            task_runner.await;
        });
        let thread_ids = Arc::new(std::sync::Mutex::new(HashSet::new()));
        let thread_ids1 = thread_ids.clone();
        spawn(async move {
            for i in 0..MAX_TASKS {
                let thread_ids1 = thread_ids1.clone();
                let mailbox = mailbox.clone();
                spawn(async move {
                    let thread_ids2 = thread_ids1.clone();
                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            sleep(std::time::Duration::from_micros(1)).await;
                            thread_ids2
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            i
                        })
                        .await;

                    //send and wait reply
                    let thread_ids3 = thread_ids1.clone();
                    let _res = mailbox
                        .spawn(async move {
                            thread_ids3
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            sleep(std::time::Duration::from_micros(10)).await;
                            i * i + 100
                        })
                        .result()
                        .await;
                });
            }
        });

        for i in 0..10 {
            log::info!(
                "[test_task_exec_queue] {}, {:?} pending_wakers_count: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                exec.pending_wakers_count(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_flushing(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(200)).await;
        }

        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASKS * 2);
        assert!(thread_ids.lock().unwrap().len() > 1);

        log::info!(
            "[test_task_exec_queue] close {:?}, thread_ids: {}, pending_wakers_count: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            thread_ids.lock().unwrap().len(),
            exec.pending_wakers_count(),
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

    let (exec, task_runner) = Builder::default()
        .workers(10)
        .queue_max(100)
        .group()
        .build::<&str>();
    let root_fut = async move {
        spawn(async {
            //start executor
            task_runner.await;
        });

        //execute task ...
        let _res = async move {
            log::info!("[test_group] hello world!");
        }
        .spawn(&exec)
        .group("g1")
        .await;

        let res = async move { "hello world!" }
            .spawn(&exec)
            .group("g1")
            .result()
            .await;
        log::info!("[test_group] result: {:?}", res.ok());

        exec.flush().await;
        log::info!(
            "[test_group] exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count()
        );
        assert!(
            exec.completed_count() == 2 && exec.waiting_count() == 0 && exec.active_count() == 0
        );
    };
    async_std::task::block_on(root_fut);
}

fn test_group_bench() {
    use rust_box::task_exec_queue::Builder;
    use tokio::{task::spawn, time::sleep};
    const MAX_TASKS: isize = 50_0000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = Builder::default()
        .workers(100)
        .queue_max(10000)
        .group()
        .build::<isize>();
    let mailbox = exec.clone();
    let runner = async move {
        spawn(async move {
            task_runner.await;
        });

        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let mailbox = mailbox.clone();
                spawn(async move {
                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            // sleep(std::time::Duration::from_nanos(1)).await;
                        })
                        .group(i % 10)
                        .await;

                    //send and wait reply
                    let _res = mailbox
                        .spawn(async move {
                            // sleep(std::time::Duration::from_nanos(1)).await;
                            i * i + 100
                        })
                        .group(i % 100)
                        .result()
                        .await;
                    // log::info!("calc: {} * {} + 100 = {:?}", i, i, _res.ok());
                });
            }
        });

        for i in 0..10 {
            log::info!(
                "[test_group_bench] {}  {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
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
        exec.flush().await.unwrap();
        exec.close().await.unwrap();
        log::info!(
            "[test_group_bench] close {:?}  pending_wakers_count: {}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.pending_wakers_count(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count(),
            exec.rate()
        );
        assert!(exec.completed_count() == MAX_TASKS * 2);
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}

fn test_local() {
    use rust_box::task_exec_queue::{LocalBuilder, LocalSpawnExt};
    use tokio::task::spawn_local;
    let (exec, task_runner) = LocalBuilder::default().workers(10).queue_max(100).build();
    let root_fut = async move {
        spawn_local(async {
            //start executor
            task_runner.await;
        });

        //execute task and return result...
        let res = async { "hello world!" }.spawn(&exec).result().await;
        log::info!("[test_local] result: {:?}", res.ok());

        exec.flush().await;
        exec.close().await;

        assert!(exec.completed_count() == 1);
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}

fn test_local_with_channel() {
    use rust_box::task_exec_queue::{LocalBuilder, LocalSpawnExt};
    use tokio::task::spawn_local;
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let (exec, task_runner) = LocalBuilder::default().with_channel(tx, rx).build();
    let root_fut = async move {
        spawn_local(async {
            //start executor
            task_runner.await;
        });

        //execute task and return result...
        let res = async { "hello world!" }.spawn(&exec).result().await;
        log::info!("[test_local_with_channel] result: {:?}", res.ok());

        exec.flush().await;
        exec.close().await;

        assert!(exec.completed_count() == 1);
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}

fn test_local_group_with_channel() {
    use rust_box::task_exec_queue::{LocalBuilder, LocalSpawnExt};
    use tokio::task::spawn_local;
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let (exec, task_runner) = LocalBuilder::default().with_channel(tx, rx).group().build();
    let root_fut = async move {
        spawn_local(async {
            //start executor
            task_runner.await;
        });

        //execute task and return result...
        let res = async { "hello world!" }
            .spawn(&exec)
            .group(1)
            .result()
            .await;
        log::info!("[test_local_group_with_channel] result: {:?}", res.ok());

        exec.flush().await;
        exec.close().await;
        assert!(exec.completed_count() == 1);
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}

fn test_local_task_exec_queue() {
    use rust_box::task_exec_queue::LocalBuilder;
    use tokio::{task::spawn_local as spawn, time::sleep};
    const MAX_TASKS: isize = 30_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = LocalBuilder::default().workers(200).queue_max(1000).build();
    let mailbox = exec.clone();
    let root_fut = async move {
        spawn(async move {
            task_runner.await;
        });
        let thread_ids = Rc::new(std::sync::Mutex::new(HashSet::new()));
        let thread_ids1 = thread_ids.clone();
        spawn(async move {
            for i in 0..MAX_TASKS {
                let mailbox = mailbox.clone();
                let thread_ids2 = thread_ids1.clone();
                let thread_ids3 = thread_ids1.clone();
                spawn(async move {
                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            thread_ids2
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            sleep(std::time::Duration::from_micros(1)).await;
                            i
                        })
                        .await;

                    //send and wait reply
                    let _res = mailbox
                        .spawn(async move {
                            thread_ids3
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            sleep(std::time::Duration::from_micros(10)).await;
                            i * i + 100
                        })
                        .result()
                        .await;
                });
            }
        });

        for i in 0..10 {
            log::info!(
                "[test_local_task_exec_queue] {}, {:?} thread_ids: {}, pending_wakers_count: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                thread_ids.lock().unwrap().len(),
                exec.pending_wakers_count(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_flushing(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(200)).await;
        }

        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASKS * 2);
        thread_ids
            .lock()
            .unwrap()
            .insert(std::thread::current().id());
        log::info!(
            "[test_local_task_exec_queue] close {:?}, thread_ids: {}, pending_wakers_count: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            thread_ids.lock().unwrap().len(),
            exec.pending_wakers_count(),
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
    //    tokio::runtime::Runtime::new().unwrap().block_on(root_fut);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}
