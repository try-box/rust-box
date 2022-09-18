use futures::SinkExt;
use rust_box::task_executor::Builder;
use tokio::{task::spawn, time::sleep};

// use async_std::{
//     future::timeout,
//     task::{sleep, spawn},
// };

fn main() {
    std::env::set_var("RUST_LOG", "tokio_executor=info,task_executor=info");
    env_logger::init();
    main_tokio();
    // main_tokio_group();
}

fn main_tokio() {
    const MAX_TASK: isize = 100_000;
    let now = std::time::Instant::now();

    let (mut exec, runner) = Builder::default().workers(100).queue_max(10_000).build();

    let mailbox = exec.mailbox();

    let runner = async move {
        spawn(async move {
            for _i in 0..MAX_TASK {
                let mut mailbox = mailbox.clone();
                spawn(async move {
                    mailbox
                        .send(async move {
                            // sleep(std::time::Duration::from_micros(1)).await;
                        })
                        .await
                        .unwrap();
                });
            }
        });

        spawn(async move {
            runner.await;
        });

        for i in 0..10 {
            println!(
                "{}  {:?} actives: {}, waitings: {}, completeds: {}, rate: {:?}",
                i,
                now.elapsed(),
                exec.active_count(),
                exec.waiting_count(),
                exec.completed_count(),
                exec.rate()
            );
            sleep(std::time::Duration::from_millis(100)).await;
        }

        exec.close().await.unwrap();

        assert!(exec.completed_count() == MAX_TASK);

        println!(
            "close {:?} actives: {}, waitings: {}, completeds: {}, rate: {:?}",
            now.elapsed(),
            exec.active_count(),
            exec.waiting_count(),
            exec.completed_count(),
            exec.rate()
        );
    };

    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
    // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}

// fn main_tokio_group() {
//
//     // use tokio::task::{spawn_local as spawn};
//
//     let runner = async move {
//         let now = std::time::Instant::now();
//         let send_count = Arc::new(AtomicIsize::new(0));
//         let send_count1 = send_count.clone();
//         let send_error_count = Arc::new(AtomicIsize::new(0));
//         let send_error_count1 = send_error_count.clone();
//         let waiting_count = Arc::new(AtomicIsize::new(0));
//         let waiting_count1 = waiting_count.clone();
//         {
//             // let (task_tx, task_rx) = mpsc::channel::<Box<dyn Future<Output=i32> + Send + Unpin>>(100);
//             let (task_tx, task_rx) = mpsc::channel::<TaskType>(100);
//
//             // let task_tx = task_tx.clone().with(|x|{
//             //     println!("xxx");
//             //     ok::<_, SendError>(x)
//             // });
//
//             //+ Send + 'static + Unpin
//             let exec = Builder::default().workers(100).build(task_tx.clone(), task_rx); //.with_group::<String, TaskType>(100);
//
//             let mut mailbox = exec.mailbox();
//
//             // let exec1 = exec.;
//
//             spawn(async move {
//                 // let res = mailbox.send(async {println!("{}", 123)}).await.unwrap();
//                 // let res = exec1.send("test".into(), async {1}).await;
//
//
//                 // let mut mailbox2 = mailbox.clone();
//                 for i in 0..10000 {
//                     // let send_count2 = send_count1.clone();
//                     // let mut mailbox3 = mailbox.clone();
//                     // let waiting_count2 = waiting_count1.clone();
//
//                     // //send ...
//                     // let res = mailbox2.clone().send(async move {
//                     //     send_count2.fetch_add(1, Ordering::SeqCst);
//                     //     // log::info!("{:?}", i);
//                     //     sleep(Duration::from_millis(1)).await;
//                     // }).await;
//
//                     //call
//
//                     // spawn(async move {
//                     //     //let waiting_count3 = waiting_count2.clone();
//                     //     send_count2.fetch_add(1, Ordering::SeqCst);
//                     //     waiting_count2.fetch_add(1, Ordering::SeqCst);
//                     //     let (rs_tx, rs_rx) = oneshot::channel();
//                     //     let task = async move {
//                     //         sleep(Duration::from_millis(100)).await;
//                     //         if let Err(_e) = rs_tx.send(1) {
//                     //             log::warn!("send result failed");
//                     //         }
//                     //     };
//                     //     let res = mailbox3.send(task).await;
//                     //     let res = rs_rx.await;
//                     //     waiting_count2.fetch_sub(1, Ordering::SeqCst);
//                     // });
//                 }
//             });
//             // sleep(Duration::from_millis(1000)).await;
//
//             for i in 0..20 {
//                 log::info!("local {}  {:?} active_count: {}, waiting_count: {}, completed_count: {}, send_count: {}, send_error_count: {}, exec_rate: {:?}",
//                  i, now.elapsed(), exec.active_count(), waiting_count.load(Ordering::SeqCst), exec.completed_count(),
//             send_count.load(Ordering::SeqCst), send_error_count.load(Ordering::SeqCst), exec.rate().await);
//
//                 sleep(std::time::Duration::from_millis(1000)).await;
//             }
//         }
//         sleep(Duration::from_millis(1000)).await;
//     };
//     // tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
//     tokio::runtime::Runtime::new().unwrap().block_on(runner);
// }
