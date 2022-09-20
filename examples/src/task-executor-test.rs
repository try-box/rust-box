#![allow(unused_must_use)]
#![allow(dead_code)]

use rust_box::task_executor::{Builder, init_default, SpawnExt};
// use tokio::{task::spawn_local as spawn, time::sleep};
use tokio::{task::spawn, time::sleep};

// use async_std::{
//     future::timeout,
//     task::{sleep, spawn},
// };

fn main() {
    std::env::set_var("RUST_LOG", "task_executor=info,test_executor_ext=info");
    env_logger::init();
    test_executor_ext();
    test_executor();
    // main_tokio_group();
}


fn test_executor_ext() {

    //init default executor
    let runner = init_default();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        spawn(async {
            //start executor
            runner.await;
        });

        //execute future ...
        let res = async {
            log::info!("execute future ...");
        }.spawn().await;
        assert_eq!(res.ok(), Some(()));

        //execute future and return result...
        let res = async {
            3 + 2 - 5 + 100
        }.spawn().result().await;
        assert_eq!(res.as_ref().ok(), Some(&100));
        log::info!("execute and result is {:?}", res.ok());

        sleep(std::time::Duration::from_millis(1000)).await;
    });
}

fn test_executor() {
    const MAX_TASKS: isize = 100_000;
    let now = std::time::Instant::now();
    let (mut exec, runner) = Builder::default().workers(150).queue_max(1000_000).build();
    let mailbox = exec.clone();
    let runner = async move {
        spawn(async move {
            for i in 0..MAX_TASKS {
                let mut mailbox = mailbox.clone();
                spawn(async move {

                    //try send
                    let _res = mailbox
                        .try_spawn(async {
                            // sleep(std::time::Duration::from_micros(1)).await;
                        });

                    //send ...
                    let _res = mailbox
                        .spawn(async move {
                            // sleep(std::time::Duration::from_micros(1)).await;
                            i
                        }).await;

                    //send and wait reply
                    let _res = mailbox.spawn(async move {
                        // sleep(std::time::Duration::from_micros(1)).await;
                        i * i + 100
                    }).result().await;

                    // log::info!("calc: {} * {} + 100 = {:?}", i, i, res.ok());
                });
            }
        });

        spawn(async move {
            runner.await;
        });

        for i in 0..10 {
            log::info!(
                "{}  {:?} actives: {}, waitings: {}, is_full: {},  closed: {}, closing: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                exec.active_count(),
                exec.waiting_count(),
                exec.is_full(),
                exec.is_closed(),
                exec.is_closing(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(100)).await;
        }

        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASKS * 3);

        log::info!(
            "close {:?} actives: {}, waitings: {}, is_full: {},  is_closed: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.active_count(),
            exec.waiting_count(),
            exec.is_full(),
            exec.is_closed(),
            exec.completed_count(),
            exec.rate()
        );
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}
