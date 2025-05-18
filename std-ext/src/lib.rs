use core::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, AtomicU64, AtomicUsize};
use std::sync::Arc;

pub use map::{CacheMapExt, EntryExt, TimedValue};
pub use wrapper::{HashExt, OrdExt, OrdHashExt};

pub mod map;
pub mod wrapper;

#[macro_export]
macro_rules! tuple_deref {
    ($Name:ty) => {
        impl<T> std::ops::Deref for $Name {
            type Target = T;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

#[macro_export]
macro_rules! tuple_deref_mut {
    ($Name:ty) => {
        impl<T> std::ops::DerefMut for $Name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

#[macro_export]
macro_rules! tuple_take {
    ($Name:ty) => {
        impl<T> $Name {
            #[inline]
            pub fn take(self) -> T {
                self.0
            }
        }
    };
}

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
fn test_atomic() {
    use std::sync::atomic::Ordering;

    assert_eq!(100usize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100isize.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100u64.atomic().load(Ordering::SeqCst), 100);
    assert_eq!(100i64.atomic().load(Ordering::SeqCst), 100);
    assert!(true.atomic().load(Ordering::SeqCst))
}
