#![allow(unused_must_use)]
#![allow(dead_code)]

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use crossbeam_queue::SegQueue;
use futures::{Sink, Stream};
use parking_lot::RwLock;
use rust_box::queue_ext::{Action, QueueExt, Reply};

fn main() {
    std::env::set_var("RUST_LOG", "task_exec_queue_test=info");
    env_logger::init();

    test_quick_start();
    test_return_result();
    test_default_set();
    test_task_exec_queue();
    test_task_exec_queue_with_priority_channel();

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
            default().completed_count().await,
            default().waiting_count(),
            default().active_count()
        );

        assert!(
            default().completed_count().await == 1
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
            exec.completed_count().await,
            exec.waiting_count(),
            exec.active_count(),
        );

        assert!(
            exec.completed_count().await == 1
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
            exec.completed_count().await
        );

        assert!(
            exec.completed_count().await == 3
                && exec.waiting_count() == 0
                && exec.active_count() == 0
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
            exec.completed_count().await
        );

        assert!(
            exec.completed_count().await == 1
                && exec.waiting_count() == 0
                && exec.active_count() == 0
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
    use rust_box::mpsc::priority_channel;
    use rust_box::task_exec_queue::{Builder, SpawnExt};
    let queue_max = 100;
    //    let (tx, rx) = channel_with_name(queue_max);
    let (tx, rx) = priority_channel(queue_max);
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
            exec.completed_count().await
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
            exec.completed_count().await
        );
        exec.flush().await;
        //        sleep(Duration::from_millis(200)).await;
        log::info!(
            "[test_channel_with_name] 4 exec.actives: {}, waitings: {}, completeds: {}",
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count().await
        );

        assert!(
            (exec.completed_count().await == 2 || exec.completed_count().await == 3)
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
    use indexmap::IndexMap;
    use parking_lot::RwLock;
    let (tx, rx) = Arc::new(RwLock::new(IndexMap::new())).queue_channel(
        move |s, act| match act {
            Action::Send((key, val)) => {
                log::info!("[channel_with_name] key: {:?}, val: {:p}", key, &val);
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
                match s.pop() {
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
    const MAX_TASKS: isize = 5_000_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = Builder::default().workers(1000).queue_max(10_000).build();
    let mailbox = exec.clone();
    let root_fut = async move {
        spawn(async move {
            task_runner.await;
        });
        let thread_ids = Arc::new(std::sync::Mutex::new(HashSet::new()));
        let thread_ids1 = thread_ids.clone();
        let is_closes = Arc::new(AtomicIsize::new(0));
        let is_closes1 = is_closes.clone();
        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let thread_ids1 = thread_ids1.clone();
                let mailbox = mailbox.clone();
                let is_closes1 = is_closes1.clone();
                spawn(async move {
                    let thread_ids2 = thread_ids1.clone();
                    let is_closes1 = is_closes1.clone();
                    //send ...

                    let res = mailbox
                        .spawn(async move {
                            //sleep(std::time::Duration::from_millis(1)).await;
                            thread_ids2
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            i
                        })
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        }
                    }

                    //send and wait reply
                    let thread_ids3 = thread_ids1.clone();
                    let res = mailbox
                        .spawn(async move {
                            thread_ids3
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            //sleep(std::time::Duration::from_millis(1)).await;
                            i * i + 100
                        })
                        .result()
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                });
            }
        });

        let exec1 = exec.clone();
        spawn(async move {
            let exec = exec1;
            let mut last = 0;
            loop {
                sleep(std::time::Duration::from_millis(1000)).await;
                let completed_count = exec.completed_count().await;
                log::info!(
                    "[test_task_exec_queue] {:?} pending_wakers: {:?}, waiting_wakers: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                    now.elapsed(),
                    exec.pending_wakers_count(),
                    exec.waiting_wakers_count(),
                    exec.active_count(),
                    exec.waiting_count(),
                    exec.is_full(),
                    exec.is_closed(),
                    exec.is_flushing(),
                    completed_count,
                    (completed_count - last) as f64 / 3.0 //exec.rate()
                );
                last = completed_count;
            }
        });

        test_spawns.await;
        //sleep(std::time::Duration::from_millis(200)).await;
        exec.flush().await.unwrap();
        exec.close().await.unwrap();

        log::info!(
            "[test_task_exec_queue] close {:?}, thread_ids: {}, pending_wakers: {:?}, waiting_wakers: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            thread_ids.lock().unwrap().len(),
            exec.pending_wakers_count(),
            exec.waiting_wakers_count(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count().await,
            exec.rate().await
        );

        let completed_count = (MAX_TASKS * 2) - is_closes.load(Ordering::SeqCst);
        assert!(
            exec.completed_count().await == completed_count,
            "completed_count: {}, {}",
            exec.completed_count().await,
            completed_count
        );
        assert!(thread_ids.lock().unwrap().len() > 1);
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(root_fut);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}

//
fn test_task_exec_queue_with_priority_channel() {
    use rust_box::std_ext::ArcExt;
    use rust_box::task_exec_queue::Builder;
    use rust_box::task_exec_queue::SpawnExt;
    use tokio::{task::spawn, time::sleep};
    const MAX_TASKS: isize = 5_000_000;
    let now = std::time::Instant::now();
    let queue = RwLock::new(rust_box::collections::PriorityQueue::default()).arc();
    let max_queue = 10_000;
    let (tx, rx) = rust_box::mpsc::with_priority_channel(queue, max_queue);
    let (exec, task_runner) = Builder::default()
        .workers(1000)
        .queue_max(max_queue)
        .with_channel(tx, rx)
        .build();
    let mailbox = exec.clone();
    let root_fut = async move {
        spawn(async move {
            task_runner.await;
        });
        let thread_ids = Arc::new(std::sync::Mutex::new(HashSet::new()));
        let thread_ids1 = thread_ids.clone();
        let is_closes = Arc::new(AtomicIsize::new(0));
        let is_closes1 = is_closes.clone();
        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let thread_ids1 = thread_ids1.clone();
                let mailbox = mailbox.clone();
                let is_closes1 = is_closes1.clone();
                spawn(async move {
                    let thread_ids2 = thread_ids1.clone();
                    let is_closes1 = is_closes1.clone();
                    //send ...

                    let res = mailbox
                        .spawn_with(
                            async move {
                                //                                sleep(std::time::Duration::from_millis(1)).await;
                                thread_ids2
                                    .lock()
                                    .unwrap()
                                    .insert(std::thread::current().id());
                                i
                            },
                            rand::random::<u8>() / 254,
                        )
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        }
                    }

                    //send and wait reply
                    let thread_ids3 = thread_ids1.clone();
                    let res = mailbox
                        .spawn_with(
                            async move {
                                thread_ids3
                                    .lock()
                                    .unwrap()
                                    .insert(std::thread::current().id());
                                //                                sleep(std::time::Duration::from_millis(1)).await;
                                i * i + 100
                            },
                            rand::random::<u8>() / 254,
                        )
                        .result()
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                });
            }
        });

        let exec1 = exec.clone();
        spawn(async move {
            let exec = exec1;
            let mut last = 0;
            loop {
                sleep(std::time::Duration::from_millis(1000)).await;
                let completed_count = exec.completed_count().await;
                log::info!(
                    "[test_task_exec_queue_with_priority_channel] {:?} pending_wakers: {:?}, waiting_wakers: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                    now.elapsed(),
                    exec.pending_wakers_count(),
                    exec.waiting_wakers_count(),
                    exec.active_count(),
                    exec.waiting_count(),
                    exec.is_full(),
                    exec.is_closed(),
                    exec.is_flushing(),
                    completed_count,
                    (completed_count - last) as f64 / 3.0 //exec.rate().await
                );
                last = completed_count;
            }
        });

        let exec2 = exec.clone();
        let quickly_completed_count = Arc::new(AtomicIsize::new(0));
        let quickly_completed_count1 = quickly_completed_count.clone();
        spawn(async move {
            loop {
                let quickly_completed_count1 = quickly_completed_count1.clone();
                async move {
                    quickly_completed_count1.fetch_add(1, Ordering::SeqCst);
                    log::info!("********** High priority: {:?}", 255);
                }
                .spawn_with(&exec2, u8::MAX)
                .quickly()
                .await;
                sleep(std::time::Duration::from_millis(5000)).await;
            }
        });

        test_spawns.await;

        //sleep(std::time::Duration::from_millis(200)).await;
        exec.flush().await.unwrap();
        exec.close().await.unwrap();

        log::info!(
            "[test_task_exec_queue_with_priority_channel] close {:?}, thread_ids: {}, pending_wakers: {:?}, waiting_wakers: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            thread_ids.lock().unwrap().len(),
            exec.pending_wakers_count(),
            exec.waiting_wakers_count(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count().await,
            exec.rate().await
        );

        let completed_count = (MAX_TASKS * 2) - is_closes.load(Ordering::SeqCst)
            + quickly_completed_count.load(Ordering::SeqCst);
        assert!(
            exec.completed_count().await == completed_count,
            "completed_count: {}, {}",
            exec.completed_count().await,
            completed_count
        );
        assert!(thread_ids.lock().unwrap().len() > 1);
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
            exec.completed_count().await
        );
        assert!(
            exec.completed_count().await == 2
                && exec.waiting_count() == 0
                && exec.active_count() == 0
        );
    };
    async_std::task::block_on(root_fut);
}

fn test_group_bench() {
    use rust_box::task_exec_queue::Builder;
    use tokio::{task::spawn, time::sleep};
    const MAX_TASKS: isize = 5_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = Builder::default()
        .workers(100)
        .queue_max(1000)
        .group()
        .build::<isize>();
    let mailbox = exec.clone();
    let runner = async move {
        spawn(async move {
            task_runner.await;
        });

        let is_closes = Arc::new(AtomicIsize::new(0));
        let is_closes1 = is_closes.clone();
        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let mailbox = mailbox.clone();
                let is_closes1 = is_closes1.clone();
                spawn(async move {
                    //send ...
                    let res = mailbox
                        .spawn(async move {
                            sleep(std::time::Duration::from_nanos(1)).await;
                        })
                        .group(i % 10)
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        } else {
                            log::warn!("{:?}", e.to_string());
                        }
                    }

                    //send and wait reply
                    let res = mailbox
                        .spawn(async move {
                            sleep(std::time::Duration::from_nanos(1)).await;
                            i * i + 100
                        })
                        .group(i % 100)
                        .result()
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        } else {
                            log::warn!("{:?}", e.to_string());
                        }
                    }
                });

                if i + 1 >= MAX_TASKS {
                    log::info!("[test_group_bench] commit group end, {}", i);
                }
            }
        });

        let exec1 = exec.clone();
        spawn(async move {
            let exec = exec1;
            loop {
                log::info!(
                "[test_group_bench] {:?} pending_wakers: {}, waiting_wakers: {}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                now.elapsed(),
                exec.pending_wakers_count(),
                exec.waiting_wakers_count(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_flushing(),
                exec.completed_count().await,
                exec.rate().await
            );
                sleep(std::time::Duration::from_millis(500)).await;
            }
        });

        test_spawns.await;
        //sleep(std::time::Duration::from_millis(200)).await;
        exec.flush().await.unwrap();
        exec.close().await.unwrap();

        let completed_count = (MAX_TASKS * 2) - is_closes.load(Ordering::SeqCst);
        assert!(
            exec.completed_count().await == completed_count,
            "completed_count: {}, {}",
            exec.completed_count().await,
            completed_count
        );

        log::info!(
            "[test_group_bench] close {:?}  pending_wakers: {}, waiting_wakers: {}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.pending_wakers_count(),
            exec.waiting_wakers_count(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count().await,
            exec.rate().await
        );
        let completed_count = (MAX_TASKS * 2) - is_closes.load(Ordering::SeqCst);
        assert!(
            exec.completed_count().await == completed_count,
            "completed_count: {}, {}",
            exec.completed_count().await,
            completed_count
        );
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

        assert!(exec.completed_count().await == 1);
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

        assert!(exec.completed_count().await == 1);
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
        assert!(exec.completed_count().await == 1);
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}

fn test_local_task_exec_queue() {
    use rust_box::task_exec_queue::LocalBuilder;
    use tokio::{task::spawn_local as spawn, time::sleep};
    const MAX_TASKS: isize = 100_000;
    let now = std::time::Instant::now();
    let (exec, task_runner) = LocalBuilder::default().workers(200).queue_max(1000).build();
    let mailbox = exec.clone();
    let root_fut = async move {
        spawn(async move {
            task_runner.await;
        });
        let thread_ids = Rc::new(std::sync::Mutex::new(HashSet::new()));
        let thread_ids1 = thread_ids.clone();
        let is_closes = Arc::new(AtomicIsize::new(0));
        let is_closes1 = is_closes.clone();
        let test_spawns = spawn(async move {
            for i in 0..MAX_TASKS {
                let mailbox = mailbox.clone();
                let thread_ids2 = thread_ids1.clone();
                let thread_ids3 = thread_ids1.clone();
                let is_closes1 = is_closes1.clone();
                spawn(async move {
                    //send ...
                    let res = mailbox
                        .spawn(async move {
                            thread_ids2
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            //sleep(std::time::Duration::from_micros(1)).await;
                            i
                        })
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        } else {
                            log::warn!("{:?}", e.to_string());
                        }
                    }

                    //send and wait reply
                    let res = mailbox
                        .spawn(async move {
                            thread_ids3
                                .lock()
                                .unwrap()
                                .insert(std::thread::current().id());
                            //sleep(std::time::Duration::from_micros(10)).await;
                            i * i + 100
                        })
                        .result()
                        .await;
                    if let Err(e) = res {
                        //log::warn!("{:?}", e.to_string());
                        if e.is_closed() {
                            is_closes1.fetch_add(1, Ordering::SeqCst);
                        } else {
                            log::warn!("{:?}", e.to_string());
                        }
                    }
                });
            }
        });

        let exec1 = exec.clone();
        let thread_ids1 = thread_ids.clone();
        spawn(async move {
            let exec = exec1;
            loop {
                log::info!(
                    "[test_local_task_exec_queue] {:?} thread_ids: {}, pending_wakers: {:?}, waiting_wakers: {:?}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
                    now.elapsed(),
                    thread_ids1.lock().unwrap().len(),
                    exec.pending_wakers_count(),
                    exec.waiting_wakers_count(),
                    exec.active_count(),
                    exec.waiting_count(),
                    exec.is_full(),
                    exec.is_closed(),
                    exec.is_flushing(),
                    exec.completed_count().await,
                    exec.rate().await
                );
                sleep(std::time::Duration::from_millis(200)).await;
            }
        });

        test_spawns.await.unwrap();
        exec.flush().await.unwrap();
        exec.close().await.unwrap();

        thread_ids
            .lock()
            .unwrap()
            .insert(std::thread::current().id());
        log::info!(
            "[test_local_task_exec_queue] close {:?}, thread_ids: {}, pending_wakers: {:?}, waiting_wakers: {}, actives: {}, waitings: {}, is_full: {},  closed: {}, flushing: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            thread_ids.lock().unwrap().len(),
            exec.pending_wakers_count(),
            exec.waiting_wakers_count(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.is_flushing(),
            exec.completed_count().await,
            exec.rate().await
        );

        let completed_count = (MAX_TASKS * 2) - is_closes.load(Ordering::SeqCst);
        assert!(
            exec.completed_count().await == completed_count,
            "completed_count: {}, {}",
            exec.completed_count().await,
            completed_count
        );
    };

    // async_std::task::block_on(runner);
    //    tokio::runtime::Runtime::new().unwrap().block_on(root_fut);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), root_fut);
}
