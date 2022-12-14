use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, AtomicU64, AtomicUsize};
pub use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard, Mutex, MutexGuard};

pub mod async_std;

impl<T: ?Sized> ArcExt for T {}

pub trait ArcExt {
    #[inline]
    fn arc(self) -> Arc<Self>
        where
            Self: Sized,
    {
        Arc::new(self)
    }
}

impl<T: ?Sized> RwLockExt for T {}

pub trait RwLockExt {

    #[inline]
    fn rwlock(self) -> RwLock<Self>
        where
            Self: Sized,
    {
        RwLock::new(self)
    }

}

impl<T: ?Sized> MutexExt for T {}

pub trait MutexExt {

    #[inline]
    fn mutex(self) -> Mutex<Self>
        where
            Self: Sized,
    {
        Mutex::new(self)
    }

}

pub trait AtomicExt<T> {
    fn atomic(self) -> T;
}

impl AtomicExt<AtomicUsize> for usize {
    #[inline]
    fn atomic(self) -> AtomicUsize {
        AtomicUsize::new(self)
    }
}

impl AtomicExt<AtomicIsize> for isize {
    #[inline]
    fn atomic(self) -> AtomicIsize {
        AtomicIsize::new(self)
    }
}

impl AtomicExt<AtomicU64> for u64 {
    #[inline]
    fn atomic(self) -> AtomicU64 {
        AtomicU64::new(self)
    }
}

impl AtomicExt<AtomicI64> for i64 {
    #[inline]
    fn atomic(self) -> AtomicI64 {
        AtomicI64::new(self)
    }
}

impl AtomicExt<AtomicBool> for bool {
    #[inline]
    fn atomic(self) -> AtomicBool {
        AtomicBool::new(self)
    }
}


#[test]
fn test_rwlock() {
    let a = 1.rwlock().arc();
    assert_eq!(*a.read(), 1);
    *a.write() = 2;
    assert_eq!(*a.read(), 2);
}

#[test]
fn test_mutex() {
    let m = 1.mutex().arc();
    assert_eq!(*m.lock(), 1);
    *m.lock() = 2;
    assert_eq!(*m.lock(), 2);
}

#[test]
fn test_atomic() {
    use std::sync::atomic::Ordering;

    assert_eq!(100usize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100isize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100u64.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100i64.atomic().load(Ordering::SeqCst), 100);
    assert!(true.atomic().load(Ordering::SeqCst))
}