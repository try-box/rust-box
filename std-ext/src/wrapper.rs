use super::{tuple_deref, tuple_deref_mut, tuple_take};
use std::hash::{Hash, Hasher};

impl<T: ?Sized> OrdExt for T {}

pub trait OrdExt {
    #[inline]
    fn eq_ord(self) -> EqOrdWrapper<Self>
    where
        Self: Sized,
    {
        EqOrdWrapper(self)
    }

    #[inline]
    fn neq_ord(self) -> NeqOrdWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdWrapper(self)
    }
}

#[derive(Clone)]
pub struct NeqOrdWrapper<T>(T);

tuple_deref!(NeqOrdWrapper<T>);
tuple_deref_mut!(NeqOrdWrapper<T>);
tuple_take!(NeqOrdWrapper<T>);

impl<T> Eq for NeqOrdWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Ord for NeqOrdWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Greater
    }
}

#[derive(Clone)]
pub struct EqOrdWrapper<T>(T);

tuple_deref!(EqOrdWrapper<T>);
tuple_deref_mut!(EqOrdWrapper<T>);
tuple_take!(EqOrdWrapper<T>);

impl<T> Eq for EqOrdWrapper<T> {}

impl<T> PartialEq<Self> for EqOrdWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> PartialOrd<Self> for EqOrdWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ordering::Equal)
    }
}

impl<T> Ord for EqOrdWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl<T: ?Sized> HashExt for T {}

pub trait HashExt {
    #[inline]
    fn hash_value(self, h: u64) -> HashWrapper<Self>
    where
        Self: Sized,
    {
        HashWrapper(self, h)
    }

    #[inline]
    fn hash_empty(self) -> EmptyHashWrapper<Self>
    where
        Self: Sized,
    {
        EmptyHashWrapper(self)
    }
}

pub struct HashWrapper<T>(T, u64);

tuple_deref!(HashWrapper<T>);
tuple_deref_mut!(HashWrapper<T>);

impl<T> Hash for HashWrapper<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}

pub struct EmptyHashWrapper<T>(T);

tuple_deref!(EmptyHashWrapper<T>);
tuple_deref_mut!(EmptyHashWrapper<T>);

impl<T> Hash for EmptyHashWrapper<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<T: ?Sized> OrdHashExt for T {}

pub trait OrdHashExt {
    #[inline]
    fn neq_ord_hash(self, h: u64) -> NeqOrdHashWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdHashWrapper(self, h)
    }

    #[inline]
    fn neq_ord_empty(self) -> NeqOrdEmptyHashWrapper<Self>
    where
        Self: Sized,
    {
        NeqOrdEmptyHashWrapper(self)
    }
}

pub struct NeqOrdEmptyHashWrapper<T>(T);

tuple_deref!(NeqOrdEmptyHashWrapper<T>);
tuple_deref_mut!(NeqOrdEmptyHashWrapper<T>);

impl<T> Eq for NeqOrdEmptyHashWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdEmptyHashWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdEmptyHashWrapper<T> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Ord for NeqOrdEmptyHashWrapper<T> {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Greater
    }
}

impl<T> Hash for NeqOrdEmptyHashWrapper<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

pub struct NeqOrdHashWrapper<T>(T, u64);

tuple_deref!(NeqOrdHashWrapper<T>);
tuple_deref_mut!(NeqOrdHashWrapper<T>);

impl<T> Eq for NeqOrdHashWrapper<T> {}

impl<T> PartialEq<Self> for NeqOrdHashWrapper<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl<T> PartialOrd<Self> for NeqOrdHashWrapper<T> {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl<T> Hash for NeqOrdHashWrapper<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.1.hash(state)
    }
}

#[test]
fn test_neq() {
    let a1 = NeqOrdWrapper(1);
    let a2 = NeqOrdWrapper(1);
    assert!(a1.ne(&a2));
}

#[test]
fn test_eq() {
    let a1 = EqOrdWrapper(1);
    let a2 = EqOrdWrapper(1);
    assert!(a1.eq(&a2));
}

#[test]
fn test_neq_ord_cmp_order() {
    // NeqOrdWrapper always returns Greater on cmp
    let a = NeqOrdWrapper(5);
    let b = NeqOrdWrapper(10);
    assert!(a.cmp(&b) == std::cmp::Ordering::Greater);
    // PartialCmp always returns None
    assert!(a.partial_cmp(&b).is_none());
    // PartialEq always returns false
    assert!(a.ne(&b));
}

#[test]
fn test_neq_ord_clone() {
    let a = NeqOrdWrapper("hello");
    let b = a.clone();
    assert_eq!(*a, "hello");
    assert_eq!(*b, "hello");
}

#[test]
fn test_neq_ord_deref_mut() {
    let mut a = NeqOrdWrapper(0);
    *a = 100;
    assert_eq!(*a, 100);
}

#[test]
fn test_neq_ord_take() {
    let a = NeqOrdWrapper(42);
    assert_eq!(a.take(), 42);
}

#[test]
fn test_eq_ord_cmp_order() {
    // EqOrdWrapper always returns Equal on cmp and partial_cmp
    let a = EqOrdWrapper(5);
    let b = EqOrdWrapper(10);
    assert!(a.cmp(&b) == std::cmp::Ordering::Equal);
    assert!(a.partial_cmp(&b) == Some(std::cmp::Ordering::Equal));
    // PartialEq always returns true
    assert!(a.eq(&b));
}

#[test]
fn test_eq_ord_clone() {
    let a = EqOrdWrapper(42);
    let b = a.clone();
    assert_eq!(*a, 42);
    assert_eq!(*b, 42);
}

#[test]
fn test_eq_ord_take() {
    let a = EqOrdWrapper(99);
    assert_eq!(a.take(), 99);
}

#[test]
fn test_hash_wrapper_trait() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut w = HashWrapper(42, 12345);
    assert_eq!(*w, 42);
    *w = 100;
    assert_eq!(*w, 100);

    // Hash with same value produces same result
    let w1 = HashWrapper("a", 999);
    let w2 = HashWrapper("b", 999);

    let mut h1 = DefaultHasher::new();
    w1.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    w2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn test_empty_hash_wrapper_trait() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let w = EmptyHashWrapper(42);
    assert_eq!(*w, 42);

    let mut h = DefaultHasher::new();
    w.hash(&mut h);
    let default_hash = DefaultHasher::new().finish();
    assert_eq!(h.finish(), default_hash);
}

#[test]
fn test_neq_ord_hash_wrapper() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = NeqOrdHashWrapper(1, 100);
    let b = NeqOrdHashWrapper(1, 100);

    // PartialEq always false
    assert!(a.ne(&b));

    // Hash considers the u64 value
    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn test_neq_ord_empty_hash_wrapper() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = NeqOrdEmptyHashWrapper(1);
    let b = NeqOrdEmptyHashWrapper(2);

    // PartialEq always false
    assert!(a.ne(&b));

    // Ord always Greater
    assert!(a.cmp(&b) == std::cmp::Ordering::Greater);

    // Hash is empty
    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    let default_hash = DefaultHasher::new().finish();
    assert_eq!(h1.finish(), default_hash);
    assert_eq!(h2.finish(), default_hash);
}

#[test]
fn test_hash_ext_hash_value() {
    let w = 42.hash_value(12345);
    assert_eq!(*w, 42);
}

#[test]
fn test_hash_ext_hash_empty() {
    let w = 42.hash_empty();
    assert_eq!(*w, 42);
}

#[test]
fn test_ord_hash_ext_neq_ord_hash_trait() {
    let a = "test".neq_ord_hash(777);
    assert_eq!(*a, "test");
}

#[test]
fn test_ord_hash_ext_neq_ord_empty_trait() {
    let a = "test".neq_ord_empty();
    assert_eq!(*a, "test");
}
