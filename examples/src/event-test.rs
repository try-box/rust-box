use std::sync::Arc;

use rust_box::event::Event;

fn main() {
    std::env::set_var("RUST_LOG", "event_notify=info");
    env_logger::init();

    test_event();
    test_event_send();
    test_event_async();
    test_event_async_send();
    test_event_block_in_place();
}

fn test_event() {
    let simple1 = Event::listen(|args, next| {
        if args == 100 {
            return args;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            1 * args
        }
    })
        .listen(|args, next| {
            if args == 300 {
                return args;
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                3 * args
            }
        })
        .finish();

    assert_eq!(simple1.fire(100), 100);
    assert_eq!(simple1.fire(300), 300);
    assert_eq!(simple1.fire(30), 90);
}

fn test_event_send() {
    let simple1 = Event::<i32, _>::listen(|args, next| {
        if args == 100 {
            return args;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            1 * args
        }
    })
        .listen(|args, next| {
            if args == 300 {
                return args;
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                3 * args
            }
        })
        .finish();

    let simple1 = Arc::new(simple1);
    std::thread::spawn(move || {
        assert_eq!(simple1.fire(100), 100);
        assert_eq!(simple1.fire(300), 300);
        assert_eq!(simple1.fire(30), 90);
    })
        .join()
        .unwrap();
}

fn test_event_async() {
    use futures_util::FutureExt;

    let simple1 = Event::<i32, _>::listen(|args: i32, next| {
        if args == 100 {
            return async move { args }.boxed_local();
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            async move { 1 * args }.boxed_local()
        }
    })
        .listen(|args: i32, next| {
            if args == 200 {
                return async move { args }.boxed_local();
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                async move { 2 * args }.boxed_local()
            }
        })
        .listen(|args: i32, next| {
            if args == 300 {
                return async move { args }.boxed_local();
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                async move { 3 * args }.boxed_local()
            }
        })
        .finish();

    let runner = async move {
        tokio::task::spawn_local(async move {
            assert_eq!(simple1.fire(10).await, 30);
            assert_eq!(simple1.fire(200).await, 200);
            assert_eq!(simple1.fire(300).await, 300);
        })
            .await
            .unwrap();
    };
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
}

fn test_event_async_send() {
    use async_std::task::spawn;
    use futures_util::FutureExt;

    let simple1 = Event::<i32, _>::listen(|args: i32, next| {
        if args == 100 {
            return async move { args }.boxed();
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            async move { 1 * args }.boxed()
        }
    })
        .listen(|args: i32, next| {
            if args == 200 {
                return async move { args }.boxed();
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                async move { 2 * args }.boxed()
            }
        })
        .listen(|args: i32, next| {
            if args == 300 {
                return async move { args }.boxed();
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                async move { 3 * args }.boxed()
            }
        })
        .finish();

    let simple1 = Arc::new(simple1);
    let runner = async move {
        spawn(async move {
            assert_eq!(simple1.fire(10).await, 30);
            assert_eq!(simple1.fire(200).await, 200);
            assert_eq!(simple1.fire(300).await, 300);
        })
            .await;
    };
    async_std::task::block_on(runner);
}

fn test_event_block_in_place() {
    use tokio::runtime::Handle;
    use tokio::task::block_in_place;
    use tokio::task::spawn;

    let simple1 = Event::<i32, _>::listen(|args: i32, next| {
        let v = block_in_place(move || {
            Handle::current().block_on(async move {
                // do something async
                args * 2
            })
        });
        if v == 200 {
            return v;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            1 * args
        }
    })
        .listen(|args: i32, next| {
            if args == 200 {
                return args;
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                2 * args
            }
        })
        .listen(|args: i32, next| {
            if args == 300 {
                return args;
            }
            if let Some(next) = next {
                next.forward(args)
            } else {
                3 * args
            }
        })
        .finish();

    let simple1 = Arc::new(simple1);
    let runner = async move {
        spawn(async move {
            assert_eq!(simple1.fire(10), 30);
            assert_eq!(simple1.fire(100), 200);
            assert_eq!(simple1.fire(200), 200);
            assert_eq!(simple1.fire(300), 300);
        })
            .await
            .unwrap();
    };
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
}
