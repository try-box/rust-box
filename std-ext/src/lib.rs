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

#[test]
fn test_arc_ext() {
    let v = 42.arc();
    assert_eq!(*v, 42);

    let s = "hello".to_string().arc();
    assert_eq!(*s, "hello");

    // Arc is cloneable
    let s2 = s.clone();
    assert_eq!(*s2, "hello");
    assert!(Arc::ptr_eq(&s, &s2));
}

#[test]
fn test_atomic_ext_usize() {
    use std::sync::atomic::Ordering;
    let a = 0usize.atomic();
    a.store(10, Ordering::SeqCst);
    assert_eq!(a.load(Ordering::SeqCst), 10);
}

#[test]
fn test_atomic_ext_isize() {
    use std::sync::atomic::Ordering;
    let a = (-5isize).atomic();
    assert_eq!(a.load(Ordering::SeqCst), -5);
}

#[test]
fn test_atomic_ext_bool() {
    use std::sync::atomic::Ordering;
    let a = false.atomic();
    assert!(!a.load(Ordering::SeqCst));
    a.store(true, Ordering::SeqCst);
    assert!(a.load(Ordering::SeqCst));
}

#[test]
fn test_tuple_deref_macro() {
    struct MyWrapper<T>(T);
    tuple_deref!(MyWrapper<T>);

    let w = MyWrapper(42);
    assert_eq!(*w, 42);
}

#[test]
fn test_tuple_deref_mut_macro() {
    struct MyWrapper<T>(T);
    tuple_deref!(MyWrapper<T>);
    tuple_deref_mut!(MyWrapper<T>);

    let mut w = MyWrapper(42);
    *w = 100;
    assert_eq!(*w, 100);
}

#[test]
fn test_tuple_take_macro() {
    struct MyWrapper<T>(T);
    tuple_take!(MyWrapper<T>);

    let w = MyWrapper(42);
    assert_eq!(w.take(), 42);
}

#[test]
fn test_wrapper_eq_ord() {
    let a = 1.eq_ord();
    let b = 2.eq_ord();
    // EqOrdWrapper always compares equal
    assert!(a.eq(&b));
    assert!(a.partial_cmp(&b) == Some(std::cmp::Ordering::Equal));
    assert!(a.cmp(&b) == std::cmp::Ordering::Equal);
}

#[test]
fn test_wrapper_neq_ord() {
    let a = 1.neq_ord();
    let b = 1.neq_ord();
    // NeqOrdWrapper never compares equal
    assert!(a.ne(&b));
    assert!(a.partial_cmp(&b).is_none());
    assert!(a.cmp(&b) == std::cmp::Ordering::Greater);
}

#[test]
fn test_wrapper_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = 42.hash_value(12345);
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    let h1 = hasher.finish();

    let b = 99.hash_value(12345);
    let mut hasher2 = DefaultHasher::new();
    b.hash(&mut hasher2);
    let h2 = hasher2.finish();

    // Same hash value → same hash output
    assert_eq!(h1, h2);

    // Different hash value → different hash output
    let c = 42.hash_value(54321);
    let mut hasher3 = DefaultHasher::new();
    c.hash(&mut hasher3);
    let h3 = hasher3.finish();
    assert_ne!(h1, h3);
}

#[test]
fn test_empty_hash_wrapper() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = 42.hash_empty();
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    let h1 = hasher.finish();

    let b = 99.hash_empty();
    let mut hasher2 = DefaultHasher::new();
    b.hash(&mut hasher2);
    let h2 = hasher2.finish();

    // Empty hash wrapper always produces the same hash (empty)
    let h_default = DefaultHasher::new().finish();
    assert_eq!(h1, h_default);
    assert_eq!(h2, h_default);
}

#[test]
fn test_ord_hash_ext_neq_ord_hash() {
    let a = 1.neq_ord_hash(100);
    let b = 1.neq_ord_hash(100);
    // NeqOrdHashWrapper never compares equal even with same values
    assert!(a.ne(&b));
}

#[test]
fn test_ord_hash_ext_neq_ord_empty() {
    let a = 1.neq_ord_empty();
    let b = 1.neq_ord_empty();
    // NeqOrdEmptyHashWrapper never compares equal
    assert!(a.ne(&b));
    assert!(a.cmp(&b) == std::cmp::Ordering::Greater);
}

#[test]
fn test_ord_ext_reverse_via_cmp() {
    // OrdExt itself doesn't have reverse(), but we verify default methods work
    let a = 5.eq_ord();
    assert!(a.cmp(&6.eq_ord()) == std::cmp::Ordering::Equal);
    assert!(5.neq_ord().cmp(&5.neq_ord()) == std::cmp::Ordering::Greater);
}
