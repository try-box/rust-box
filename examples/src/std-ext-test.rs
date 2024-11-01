use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use rust_box::std_ext::ArcExt;

fn main() {
    std::env::set_var("RUST_LOG", "std-ext=info");
    env_logger::init();

    test_std_arc();
    test_rwlock();
    test_mutex();
    test_atomic();
    // test_async_rwlock();
    // test_async_mutex();
}

fn test_std_arc() {
    let arc_a = Arc::new(1);
    let arc_b = 1.arc();
    assert_eq!(arc_a, arc_b);
}

fn test_rwlock() {
    let a = RwLock::new(1).arc();
    assert_eq!(*a.read(), 1);
    *a.write() = 2;
    assert_eq!(*a.read(), 2);
}

fn test_mutex() {
    let m = Mutex::new(1).arc();
    assert_eq!(*m.lock(), 1);
    *m.lock() = 2;
    assert_eq!(*m.lock(), 2);
}

fn test_atomic() {
    use rust_box::std_ext::AtomicExt;
    use std::sync::atomic::Ordering;
    assert_eq!(100usize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100isize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100u64.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100i64.atomic().load(Ordering::SeqCst), 100);
    assert!(true.atomic().load(Ordering::SeqCst))
}

// fn test_async_rwlock() {
//     use rust_box::std_ext::async_lock::RwLockExt;
//     let runner = async move {
//         let a = 1.rwlock().arc();
//         assert_eq!(*a.read().await, 1);
//         *a.write().await = 2;
//         assert_eq!(*a.read().await, 2);
//     };
//     async_std::task::block_on(runner);
// }

// fn test_async_mutex() {
//     use rust_box::std_ext::async_lock::MutexExt;
//     let runner = async move {
//         let m = 1.mutex().arc();
//         assert_eq!(*m.lock().await, 1);
//         *m.lock().await = 2;
//         assert_eq!(*m.lock().await, 2);
//     };
//     async_std::task::block_on(runner);
// }
