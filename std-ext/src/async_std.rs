pub use async_lock::{Mutex, RwLock};

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
